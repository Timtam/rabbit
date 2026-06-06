//! Elevated process launcher.
//!
//! On Windows, some vendor installers — notably the JAWS-for-REAPER scripts
//! NSIS package — declare `RequestExecutionLevel admin` in their script
//! because they need to write into `C:\Program Files (x86)\…`. A normal
//! `CreateProcess` call from an unelevated parent never triggers UAC, so on
//! default Windows the installer silently no-ops in `/S` silent mode. We
//! work around that by launching the installer through `ShellExecuteExW`
//! with the `runas` verb, which always raises the UAC consent dialog when
//! the user is not already elevated, then waiting on the returned process
//! handle for the exit code.
//!
//! On macOS we wrap the same call through `osascript`'s `do shell script
//! "..." with administrator privileges` so the system raises its native
//! AuthorizationServices dialog. That's the screen-reader-friendly path
//! used by Apple's own tooling — sudo can't read passwords from a GUI
//! parent and `SMJobBless` would require a separate signed helper bundle
//! RABBIT doesn't have. Today only Surge XT's `installer -pkg` flow uses
//! this path.
//!
//! Other targets compile to a stub that returns an `Unsupported` error.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ElevationError {
    /// `ShellExecuteExW` failed before producing a process handle. `code` is
    /// the OS-reported `GetLastError()` value.
    LaunchFailed { program: PathBuf, code: u32 },
    /// `WaitForSingleObject` or `GetExitCodeProcess` failed, or the process
    /// terminated abnormally so we have no exit code to report.
    WaitFailed { program: PathBuf, message: String },
    /// The user dismissed the UAC consent prompt. Distinct from a generic
    /// launch failure so the caller can surface a clearer message.
    UserCancelledElevation { program: PathBuf },
    /// Compiled on a target that has no elevation primitive (only Windows
    /// supports this code path today).
    Unsupported,
}

impl std::fmt::Display for ElevationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LaunchFailed { program, code } => write!(
                f,
                "ShellExecuteExW(runas) failed for {} (Win32 error {code})",
                program.display()
            ),
            Self::WaitFailed { program, message } => write!(
                f,
                "could not read exit status for elevated process {}: {message}",
                program.display()
            ),
            Self::UserCancelledElevation { program } => write!(
                f,
                "the Windows administrator approval prompt for {} was cancelled or declined",
                program.display()
            ),
            Self::Unsupported => write!(
                f,
                "elevated process launch is not supported on this platform"
            ),
        }
    }
}

impl std::error::Error for ElevationError {}

/// Launch `program` with `arguments` under UAC elevation and block until it
/// exits. Returns the process exit code (`Some(n)`) on a clean exit, or
/// `None` if the OS could not return one (rare). Working directory may be
/// `None` to inherit the current directory.
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn run_elevated_and_wait(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
) -> Result<Option<i32>, ElevationError> {
    platform_run_elevated_and_wait(program, arguments, working_directory)
}

#[cfg(windows)]
fn platform_run_elevated_and_wait(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
) -> Result<Option<i32>, ElevationError> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError, WAIT_FAILED};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, INFINITE, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let verb_w: Vec<u16> = OsStr::new("runas").encode_wide().chain(Some(0)).collect();
    let program_w: Vec<u16> = program.as_os_str().encode_wide().chain(Some(0)).collect();
    let parameters = quote_arguments(arguments);
    let parameters_w: Vec<u16> = OsStr::new(&parameters)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let working_directory_w: Option<Vec<u16>> =
        working_directory.map(|path| path.as_os_str().encode_wide().chain(Some(0)).collect());

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: std::ptr::null_mut(),
        lpVerb: verb_w.as_ptr(),
        lpFile: program_w.as_ptr(),
        lpParameters: if parameters.is_empty() {
            std::ptr::null()
        } else {
            parameters_w.as_ptr()
        },
        lpDirectory: working_directory_w
            .as_ref()
            .map(|w| w.as_ptr())
            .unwrap_or(std::ptr::null()),
        nShow: SW_SHOWNORMAL,
        hInstApp: std::ptr::null_mut(),
        lpIDList: std::ptr::null_mut(),
        lpClass: std::ptr::null(),
        hkeyClass: std::ptr::null_mut(),
        dwHotKey: 0,
        Anonymous: unsafe { std::mem::zeroed() },
        hProcess: std::ptr::null_mut(),
    };

    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
        let code = unsafe { GetLastError() };
        // Windows returns ERROR_CANCELLED (1223) when the user dismisses the
        // UAC consent prompt; surface that as a distinct error so the wizard
        // can prompt them to re-run and approve.
        if code == ERROR_CANCELLED {
            return Err(ElevationError::UserCancelledElevation {
                program: program.to_path_buf(),
            });
        }
        return Err(ElevationError::LaunchFailed {
            program: program.to_path_buf(),
            code,
        });
    }

    if info.hProcess.is_null() {
        // Some shell verbs return success without a process handle (e.g.
        // when the file extension was handled by the shell instead of by
        // CreateProcess). For our use case — launching a real `.exe` — that
        // would be a misconfiguration; treat it as a wait failure so the
        // caller doesn't silently report success.
        return Err(ElevationError::WaitFailed {
            program: program.to_path_buf(),
            message: "ShellExecuteExW returned no process handle".to_string(),
        });
    }

    let wait_status = unsafe { WaitForSingleObject(info.hProcess, INFINITE) };
    if wait_status == WAIT_FAILED {
        let code = unsafe { GetLastError() };
        unsafe {
            CloseHandle(info.hProcess);
        }
        return Err(ElevationError::WaitFailed {
            program: program.to_path_buf(),
            message: format!("WaitForSingleObject failed (Win32 error {code})"),
        });
    }

    let mut exit_code: u32 = 0;
    let ok = unsafe { GetExitCodeProcess(info.hProcess, &mut exit_code) };
    unsafe {
        CloseHandle(info.hProcess);
    }
    if ok == 0 {
        let code = unsafe { GetLastError() };
        return Err(ElevationError::WaitFailed {
            program: program.to_path_buf(),
            message: format!("GetExitCodeProcess failed (Win32 error {code})"),
        });
    }

    Ok(Some(exit_code as i32))
}

#[cfg(target_os = "macos")]
fn platform_run_elevated_and_wait(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
) -> Result<Option<i32>, ElevationError> {
    use std::process::Command;

    // Re-emit the command as an AppleScript `do shell script` literal.
    // `osascript` is the standard way to raise macOS' native admin prompt
    // without bundling a signed helper. The script string must escape any
    // embedded double-quotes; everything else (spaces, colons, parens) is
    // safe inside a quoted AppleScript string.
    let command_line = if arguments.is_empty() {
        applescript_quote(&program.display().to_string())
    } else {
        let joined = std::iter::once(program.display().to_string())
            .chain(arguments.iter().cloned())
            .map(|argument| applescript_quote(&argument))
            .collect::<Vec<_>>()
            .join(" & space & ");
        joined
    };
    let script = format!(
        "do shell script ({command_line}) with administrator privileges",
        command_line = command_line
    );

    let mut command = Command::new("/usr/bin/osascript");
    command.arg("-e").arg(&script);
    if let Some(working_directory) = working_directory {
        command.current_dir(working_directory);
    }

    let output = command
        .output()
        .map_err(|err| ElevationError::LaunchFailed {
            program: program.to_path_buf(),
            code: err.raw_os_error().unwrap_or(0) as u32,
        })?;

    if output.status.success() {
        return Ok(Some(0));
    }

    // osascript exit 1 + stderr containing "User cancelled" → the user
    // dismissed the system admin prompt. Surface as a distinct error so
    // the wizard can prompt them to re-run and approve, matching the
    // Windows ERROR_CANCELLED branch.
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("User canceled") || stderr.contains("User cancelled") {
        return Err(ElevationError::UserCancelledElevation {
            program: program.to_path_buf(),
        });
    }

    Ok(output.status.code())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_run_elevated_and_wait(
    _program: &Path,
    _arguments: &[String],
    _working_directory: Option<&Path>,
) -> Result<Option<i32>, ElevationError> {
    Err(ElevationError::Unsupported)
}

/// Escape a literal command-line token for embedding inside an
/// AppleScript double-quoted string. Internal `"` and `\` are escaped so
/// the resulting string round-trips through `do shell script` exactly.
#[cfg(target_os = "macos")]
fn applescript_quote(argument: &str) -> String {
    let escaped = argument.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Quote each argument the way `ShellExecuteEx` expects (one space-joined
/// command-line string), wrapping arguments containing whitespace or quotes
/// in double-quotes and escaping internal quotes.
fn quote_arguments(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|argument| quote_one(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Quote a single argument for the `lpParameters` string passed to
/// `ShellExecuteExW`.
///
/// Follows the Windows command-line quoting rules: only backslashes
/// *immediately preceding a `"`* need to be doubled; backslashes elsewhere
/// are literal and must **not** be doubled.
///
/// Exception: NSIS installers parse `/D=<path>` directly from the raw
/// `GetCommandLine()` string and require the path to be the last token,
/// **unquoted** — quoting it causes NSIS to include the surrounding `"` in
/// the directory name, which produces an invalid path.  `/D=…` arguments
/// are therefore returned verbatim regardless of spaces or backslashes.
fn quote_one(argument: &str) -> String {
    // NSIS: /D=<path> must be unquoted and last in the parameter string.
    if argument.len() >= 3 && argument[..3].eq_ignore_ascii_case("/D=") {
        return argument.to_string();
    }

    // Arguments with no whitespace or embedded quotes need no quoting.
    if !argument.is_empty() && !argument.contains(|ch: char| ch.is_whitespace() || ch == '"') {
        return argument.to_string();
    }

    // Wrap in double-quotes.  Only double the backslashes that immediately
    // precede a `"` (either an explicit one in the argument or the closing
    // quote we add); backslashes elsewhere pass through unchanged.
    let mut result = String::from('"');
    let mut pending_backslashes: usize = 0;
    for ch in argument.chars() {
        match ch {
            '\\' => pending_backslashes += 1,
            '"' => {
                // N backslashes before `"` → 2*N backslashes then `\"`.
                for _ in 0..(pending_backslashes * 2) {
                    result.push('\\');
                }
                pending_backslashes = 0;
                result.push_str("\\\"");
            }
            _ => {
                // Backslashes not before `"` are literal.
                for _ in 0..pending_backslashes {
                    result.push('\\');
                }
                pending_backslashes = 0;
                result.push(ch);
            }
        }
    }
    // N trailing backslashes precede the closing `"` — double them.
    for _ in 0..(pending_backslashes * 2) {
        result.push('\\');
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::quote_arguments;

    #[test]
    fn quotes_arguments_with_whitespace() {
        // A regular argument with spaces is wrapped in quotes; backslashes
        // inside it are not doubled (Windows only needs that before a `"`).
        let line = quote_arguments(&[
            "/S".to_string(),
            "some arg with spaces and C:\\backslash".to_string(),
        ]);
        assert_eq!(line, "/S \"some arg with spaces and C:\\backslash\"");
    }

    #[test]
    fn nsis_d_flag_is_passed_unquoted() {
        // NSIS reads /D= from GetCommandLine() and requires the path to be
        // unquoted and last.  Quoting it causes NSIS to include the `"` in
        // the directory name, producing an invalid path.
        let line = quote_arguments(&[
            "/S".to_string(),
            "/D=C:\\Program Files (x86)\\Foo".to_string(),
        ]);
        assert_eq!(line, "/S /D=C:\\Program Files (x86)\\Foo");
    }

    #[test]
    fn skips_quoting_for_simple_arguments() {
        assert_eq!(quote_arguments(&["/S".to_string()]), "/S");
        assert_eq!(quote_arguments(&[]), "");
    }
}
