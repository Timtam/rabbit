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
//! The shell behind `do shell script` splits words and expands `$`,
//! backticks and quotes, and it does so as root. So every word RABBIT
//! hands it is POSIX single-quoted first, and only then is the whole
//! command put into one AppleScript string. The text goes straight to
//! `osascript`; no script file is written, because a file in a temporary
//! folder could be rewritten by another process of the same user between
//! RABBIT writing it and root running it.
//!
//! Other targets compile to a stub that returns an `Unsupported` error.

#[cfg(windows)]
use std::ffi::OsStr;
use std::fmt::Write as _;
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
    /// RABBIT would not hand this command to a root shell, and nothing ran.
    /// `reason` says which rule it broke (see [`run_elevated_shell_lines`]).
    RefusedCommand {
        program: PathBuf,
        reason: &'static str,
    },
    /// Compiled on a target that has no elevation primitive for this call:
    /// none at all outside Windows and macOS, and no shell lines outside
    /// macOS.
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
            Self::RefusedCommand { program, reason } => write!(
                f,
                "refused to run {} as administrator: {reason}",
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

/// Whether the elevated child is allowed to put a window on screen.
///
/// This is not cosmetic on Windows. RABBIT's release GUI build owns no
/// console, so a console-subsystem child launched with a visible show
/// command gets a console window of its own — which is what made a
/// PowerShell window flash up mid-install while RABBIT added its Defender
/// exclusion. The UAC consent dialog is drawn by the system on its own
/// desktop and is unaffected by either value, so hiding the child never
/// hides the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatedWindow {
    /// The child owns UI the user is meant to see. Vendor installers use
    /// this: even the silent ones may fall back to their own progress or
    /// error windows, and suppressing those would leave the user staring
    /// at a wizard that had apparently stopped.
    Show,
    /// The child is a console helper doing bookkeeping on RABBIT's behalf,
    /// with nothing to show and nothing to read.
    Hidden,
}

#[cfg(windows)]
impl ElevatedWindow {
    /// The `nShow` value `ShellExecuteExW` expects for this choice.
    fn show_command(self) -> i32 {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};

        match self {
            Self::Show => SW_SHOWNORMAL,
            Self::Hidden => SW_HIDE,
        }
    }
}

/// Launch `program` with `arguments` under UAC elevation and block until it
/// exits. Returns the process exit code (`Some(n)`) on a clean exit, or
/// `None` if the OS could not return one (rare). Working directory may be
/// `None` to inherit the current directory.
///
/// `window` says whether the child may show a window; see
/// [`ElevatedWindow`]. It never affects the UAC prompt itself.
///
/// On macOS the program runs in a root shell that starts in
/// `working_directory`, or in `/` when it is `None`, and the rules of
/// [`run_elevated_shell_lines`] apply: an absolute program path, no line
/// breaks or NUL, every word passed exactly as given.
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn run_elevated_and_wait(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    window: ElevatedWindow,
) -> Result<Option<i32>, ElevationError> {
    platform_run_elevated_and_wait(program, arguments, working_directory, window)
}

/// One command for [`run_elevated_shell_lines`].
///
/// `program` must be an absolute path, so the root shell never looks a
/// name up on `PATH`. Each argument reaches the program exactly as
/// written, spaces, quotes and `$` included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommand {
    pub program: PathBuf,
    pub arguments: Vec<String>,
}

impl ShellCommand {
    pub fn new<I, S>(program: impl Into<PathBuf>, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            program: program.into(),
            arguments: arguments.into_iter().map(Into::into).collect(),
        }
    }
}

/// How far a [`run_elevated_shell_lines`] run got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellLinesOutcome {
    /// Every command ran and exited with status 0.
    Completed,
    /// The command at `index` (counting from 0, in the order given)
    /// failed. Every command before it succeeded, and none after it ran.
    FailedAt { index: usize },
    /// The shell ended without naming a command. Status 100 means it could
    /// not change to `/`, so nothing ran; with anything else, RABBIT can't
    /// tell which commands ran.
    Stopped { code: Option<i32> },
}

/// Exit status of the root shell when it can't change to its working
/// directory. Nothing else has run at that point.
const SHELL_CD_FAILED_STATUS: i32 = 100;

/// Exit status when the first command of [`run_elevated_shell_lines`]
/// fails; the command at index `n` exits with this plus `n`. Each
/// command's own status is replaced, so a program that happens to exit
/// with 101 can't be mistaken for a step.
const SHELL_FIRST_STEP_STATUS: i32 = 101;

/// Shell exit statuses end at 255, which caps how many commands one
/// prompt can carry while still saying which one failed.
const SHELL_MAX_STEPS: usize = (255 - SHELL_FIRST_STEP_STATUS + 1) as usize;

/// Run `commands` one after another in a single root shell, behind one
/// administrator prompt, and stop at the first that fails.
///
/// The shell first changes to `/`, then runs each command with every word
/// POSIX single-quoted. Commands are refused before anything runs if a
/// program path isn't absolute, if any word contains a line break or NUL,
/// or if there are more than 155 of them. Callers that remove files run
/// their own root guard on these exact strings first: this function only
/// makes sure the shell receives them unchanged.
///
/// An empty list returns [`ShellLinesOutcome::Completed`] without asking
/// for a password. macOS only; other platforms return `Unsupported`.
pub fn run_elevated_shell_lines(
    commands: &[ShellCommand],
) -> Result<ShellLinesOutcome, ElevationError> {
    platform_run_elevated_shell_lines(commands)
}

#[cfg(target_os = "macos")]
fn platform_run_elevated_shell_lines(
    commands: &[ShellCommand],
) -> Result<ShellLinesOutcome, ElevationError> {
    let Some(first) = commands.first() else {
        return Ok(ShellLinesOutcome::Completed);
    };
    let script = administrator_shell_script(None, commands, ShellSteps::Numbered)?;
    let code = run_administrator_script(&script, &first.program)?;
    Ok(shell_lines_outcome(code, commands.len()))
}

#[cfg(not(target_os = "macos"))]
fn platform_run_elevated_shell_lines(
    _commands: &[ShellCommand],
) -> Result<ShellLinesOutcome, ElevationError> {
    Err(ElevationError::Unsupported)
}

#[cfg(windows)]
fn platform_run_elevated_and_wait(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    window: ElevatedWindow,
) -> Result<Option<i32>, ElevationError> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError, WAIT_FAILED};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, INFINITE, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };

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
        nShow: window.show_command(),
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
    window: ElevatedWindow,
) -> Result<Option<i32>, ElevationError> {
    // `window` has no counterpart here: `do shell script … with
    // administrator privileges` runs the command with no terminal at all,
    // which is already what `ElevatedWindow::Hidden` asks for, and macOS
    // vendor installers are `.pkg` files driven by `installer(8)` rather
    // than windows RABBIT could show or hide.
    let _ = window;
    let command = ShellCommand {
        program: program.to_path_buf(),
        arguments: arguments.to_vec(),
    };
    // Unnumbered: with a single command, the shell's status is the
    // program's own, which is what callers report.
    let script = administrator_shell_script(
        working_directory,
        std::slice::from_ref(&command),
        ShellSteps::Unnumbered,
    )?;
    run_administrator_script(&script, program)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_run_elevated_and_wait(
    _program: &Path,
    _arguments: &[String],
    _working_directory: Option<&Path>,
    _window: ElevatedWindow,
) -> Result<Option<i32>, ElevationError> {
    Err(ElevationError::Unsupported)
}

/// Hand `script` to `osascript`, which raises the system's administrator
/// prompt, and return the root shell's exit status. `program` only names
/// the command in errors.
#[cfg(target_os = "macos")]
fn run_administrator_script(script: &str, program: &Path) -> Result<Option<i32>, ElevationError> {
    use std::process::Command;

    let mut command = Command::new("/usr/bin/osascript");
    command.arg("-e").arg(script);
    // The admin shell and the installer it runs inherit this environment.
    crate::process::without_rabbit_secrets(&mut command);

    let output = command
        .output()
        .map_err(|err| ElevationError::LaunchFailed {
            program: program.to_path_buf(),
            code: err.raw_os_error().unwrap_or(0) as u32,
        })?;

    match administrator_shell_result(
        output.status.success(),
        output.status.code(),
        &String::from_utf8_lossy(&output.stderr),
    ) {
        // The user dismissed the system admin prompt. Surface as a distinct
        // error so the wizard can prompt them to re-run and approve,
        // matching the Windows ERROR_CANCELLED branch.
        AdministratorShellResult::Cancelled => Err(ElevationError::UserCancelledElevation {
            program: program.to_path_buf(),
        }),
        AdministratorShellResult::Exited(code) => Ok(code),
    }
}

/// Whether [`administrator_shell_script`] makes each command name its own
/// step on failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum ShellSteps {
    /// Each command is followed by `|| exit <n>`, so the shell's status
    /// says which one failed and nothing after it runs.
    Numbered,
    /// The commands run as written, and the shell's status is the last
    /// one's own.
    Unnumbered,
}

/// Build the AppleScript that runs `commands` in one root shell.
///
/// The shell first changes to `working_directory`, or `/` when there is
/// none, so no command depends on whatever folder RABBIT itself was
/// started from. Every word is POSIX single-quoted, which the shell takes
/// literally whatever it holds, and the whole shell text is then escaped
/// once as an AppleScript string.
///
/// Kept free of any macOS call so the exact text can be tested on every
/// platform.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn administrator_shell_script(
    working_directory: Option<&Path>,
    commands: &[ShellCommand],
    steps: ShellSteps,
) -> Result<String, ElevationError> {
    let first_program = commands
        .first()
        .map(|command| command.program.clone())
        .unwrap_or_default();
    let refuse = |program: &Path, reason: &'static str| ElevationError::RefusedCommand {
        program: program.to_path_buf(),
        reason,
    };
    if steps == ShellSteps::Numbered && commands.len() > SHELL_MAX_STEPS {
        return Err(refuse(
            &first_program,
            "too many commands for one administrator prompt",
        ));
    }

    let directory = match working_directory {
        Some(directory) => absolute_shell_path(directory)
            .ok_or_else(|| refuse(&first_program, "the working directory is not usable"))?,
        None => "/",
    };
    let mut shell = format!(
        "cd {} || exit {SHELL_CD_FAILED_STATUS}",
        posix_quote(directory)
    );

    for (index, command) in commands.iter().enumerate() {
        let program = absolute_shell_path(&command.program).ok_or_else(|| {
            refuse(
                &command.program,
                "the program is not an absolute path the shell can take as it is",
            )
        })?;
        if command
            .arguments
            .iter()
            .any(|argument| !is_shell_safe_word(argument))
        {
            return Err(refuse(
                &command.program,
                "an argument contains a line break or NUL character",
            ));
        }

        shell.push_str("; ");
        shell.push_str(&posix_quote(program));
        for argument in &command.arguments {
            shell.push(' ');
            shell.push_str(&posix_quote(argument));
        }
        if steps == ShellSteps::Numbered {
            // `index` is capped by SHELL_MAX_STEPS above, so this fits.
            let status = SHELL_FIRST_STEP_STATUS + index as i32;
            let _ = write!(shell, " || exit {status}");
        }
    }

    Ok(format!(
        "do shell script {} with administrator privileges",
        applescript_quote(&shell)
    ))
}

/// The path as text the root shell can take literally, or `None` if it
/// isn't absolute, isn't valid UTF-8, or holds a character RABBIT refuses.
///
/// Checked as text rather than with `Path::is_absolute`, so the macOS rule
/// holds when the tests run on Windows too.
fn absolute_shell_path(path: &Path) -> Option<&str> {
    path.to_str()
        .filter(|text| text.starts_with('/') && is_shell_safe_word(text))
}

/// Line breaks are refused outright rather than quoted: they are legal
/// inside single quotes, but no real path RABBIT handles contains one, and
/// refusing them keeps each command on one line the root shell reads as
/// a whole. NUL can't reach a program at all and would cut the word short.
fn is_shell_safe_word(word: &str) -> bool {
    !word.contains(['\n', '\r', '\0'])
}

/// Quote `word` for a POSIX shell. Inside single quotes nothing is special,
/// not spaces, `$`, backticks, `\` or `"`, so the only thing to handle is a
/// `'` itself: close the quotes, add an escaped `'`, and open them again.
fn posix_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// Escape `text` as an AppleScript double-quoted string. Internal `"` and
/// `\` are escaped so the text reaches `do shell script` exactly.
fn applescript_quote(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// What `osascript` running `do shell script … with administrator
/// privileges` reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdministratorShellResult {
    /// The user dismissed the administrator prompt; nothing ran.
    Cancelled,
    /// The shell's exit status, or `osascript`'s own when the shell's
    /// can't be read.
    Exited(Option<i32>),
}

/// AppleScript's error number for "User canceled", the same in every
/// system language.
const APPLESCRIPT_USER_CANCELED: i32 = -128;

/// Read the outcome from `osascript`'s exit status and error output.
///
/// `osascript` exits with 1 whatever the shell's status was, so the real
/// status comes from the error it prints, which ends with the error
/// number: `0:42: execution error: <message> (<number>)`. For a shell that
/// exited non-zero, that number is its exit status.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn administrator_shell_result(
    success: bool,
    osascript_status: Option<i32>,
    stderr: &str,
) -> AdministratorShellResult {
    if success {
        return AdministratorShellResult::Exited(Some(0));
    }
    let number = applescript_error_number(stderr);
    // The English text is only a fallback for an error with no number; the
    // number is what works on a system in another language, and it keeps a
    // program that merely prints those words from counting as a cancel.
    if number == Some(APPLESCRIPT_USER_CANCELED)
        || (number.is_none()
            && (stderr.contains("User canceled") || stderr.contains("User cancelled")))
    {
        return AdministratorShellResult::Cancelled;
    }
    match number {
        Some(status @ 1..=255) => AdministratorShellResult::Exited(Some(status)),
        // Any other number is an AppleScript error, not a shell status.
        _ => AdministratorShellResult::Exited(osascript_status),
    }
}

/// The number at the end of an `osascript` error: the `-128` in
/// `0:28: execution error: User canceled. (-128)`.
fn applescript_error_number(stderr: &str) -> Option<i32> {
    let inner = stderr.trim_end().strip_suffix(')')?;
    let (_, number) = inner.rsplit_once('(')?;
    number.parse().ok()
}

/// Which step a [`run_elevated_shell_lines`] status names.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn shell_lines_outcome(code: Option<i32>, command_count: usize) -> ShellLinesOutcome {
    match code {
        Some(0) => ShellLinesOutcome::Completed,
        Some(status) if status >= SHELL_FIRST_STEP_STATUS => {
            let index = (status - SHELL_FIRST_STEP_STATUS) as usize;
            if index < command_count {
                ShellLinesOutcome::FailedAt { index }
            } else {
                ShellLinesOutcome::Stopped { code }
            }
        }
        _ => ShellLinesOutcome::Stopped { code },
    }
}

/// Quote each argument the way `ShellExecuteEx` expects (one space-joined
/// command-line string), wrapping arguments containing whitespace or quotes
/// in double-quotes and escaping internal quotes.
#[cfg_attr(not(windows), allow(dead_code))]
fn quote_arguments(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|argument| quote_one(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Is this the NSIS `/D=<path>` destination argument?
///
/// NSIS reads it straight out of `GetCommandLine()` rather than from the
/// parsed argument vector, takes everything after `/D=` literally to the end
/// of the line, and so requires it to be **last and unquoted**. Any quoting
/// added on our behalf — by this module for `ShellExecuteExW`, or by
/// `std::process::Command` when it builds a Windows command line — ends up
/// inside the directory name and the installer silently goes somewhere else.
///
/// Every path that launches an installer has to honour this, so the rule
/// lives here rather than being restated at each call site.
pub fn is_nsis_destination_argument(argument: &str) -> bool {
    argument.len() >= 3 && argument[..3].eq_ignore_ascii_case("/D=")
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
    if is_nsis_destination_argument(argument) {
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
    #[cfg(windows)]
    use super::ElevatedWindow;
    use super::{
        AdministratorShellResult, ElevationError, ShellCommand, ShellLinesOutcome, ShellSteps,
        administrator_shell_result, administrator_shell_script, posix_quote, quote_arguments,
        shell_lines_outcome,
    };
    use std::path::Path;

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

    /// The script the macOS `run_elevated_and_wait` builds for one program.
    fn single_command_script(program: &str, arguments: &[&str]) -> String {
        administrator_shell_script(
            None,
            &[ShellCommand::new(program, arguments.iter().copied())],
            ShellSteps::Unnumbered,
        )
        .unwrap()
    }

    fn refusal_reason(result: Result<String, ElevationError>) -> &'static str {
        match result {
            Err(ElevationError::RefusedCommand { reason, .. }) => reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// Undo the AppleScript escaping, then split the shell text into words
    /// the way `/bin/sh` would. Only the little RABBIT ever leaves outside
    /// single quotes is accepted there; a `$`, backtick, `"` or blank that
    /// escaped the quoting fails the test instead of being guessed at.
    fn words_the_shell_sees(script: &str) -> Vec<String> {
        let literal = script
            .strip_prefix("do shell script \"")
            .and_then(|rest| rest.strip_suffix("\" with administrator privileges"))
            .expect("script has the do shell script frame");
        let mut shell = String::new();
        let mut chars = literal.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '\\' => shell.push(chars.next().expect("AppleScript escape is complete")),
                '"' => panic!("a bare double quote ends the AppleScript string early"),
                _ => shell.push(ch),
            }
        }

        let mut words = Vec::new();
        let mut word: Option<String> = None;
        let mut chars = shell.chars();
        while let Some(ch) = chars.next() {
            match ch {
                ' ' => words.extend(word.take()),
                ';' => {
                    words.extend(word.take());
                    words.push(";".to_string());
                }
                '\'' => {
                    let word = word.get_or_insert_with(String::new);
                    let mut closed = false;
                    for quoted in chars.by_ref() {
                        if quoted == '\'' {
                            closed = true;
                            break;
                        }
                        word.push(quoted);
                    }
                    assert!(closed, "single quote is never closed in {shell}");
                }
                '\\' => word
                    .get_or_insert_with(String::new)
                    .push(chars.next().expect("shell escape is complete")),
                _ if ch.is_ascii_alphanumeric() || ch == '|' => {
                    word.get_or_insert_with(String::new).push(ch)
                }
                _ => panic!("{ch:?} reaches the shell unquoted in {shell}"),
            }
        }
        words.extend(word);
        words
    }

    #[test]
    fn posix_quote_closes_and_reopens_around_a_single_quote() {
        assert_eq!(posix_quote("plain"), "'plain'");
        assert_eq!(posix_quote("it's"), r"'it'\''s'");
        assert_eq!(posix_quote(""), "''");
    }

    /// Volume and folder names on a disk image can contain spaces, and so
    /// can app bundles. Before the fix, the root shell split such a path at
    /// every space.
    #[test]
    fn administrator_script_keeps_a_path_with_spaces_as_one_word() {
        let script = single_command_script(
            "/usr/sbin/installer",
            &[
                "-pkg",
                "/Volumes/Surge XT/Surge XT/surge-xt-macOS.pkg",
                "-target",
                "/",
            ],
        );
        assert_eq!(
            script,
            r#"do shell script "cd '/' || exit 100; '/usr/sbin/installer' '-pkg' '/Volumes/Surge XT/Surge XT/surge-xt-macOS.pkg' '-target' '/'" with administrator privileges"#
        );

        let script =
            single_command_script("/Applications/Surge XT.app/Contents/MacOS/Surge XT", &[]);
        assert_eq!(
            script,
            r#"do shell script "cd '/' || exit 100; '/Applications/Surge XT.app/Contents/MacOS/Surge XT'" with administrator privileges"#
        );
    }

    #[test]
    fn administrator_script_escapes_a_single_quote_for_the_shell_then_for_applescript() {
        // The shell needs `'\''`, and AppleScript then needs that `\` doubled.
        let script = single_command_script(
            "/usr/sbin/installer",
            &["-pkg", "/Volumes/Toni's Disk/a.pkg"],
        );
        assert_eq!(
            script,
            r#"do shell script "cd '/' || exit 100; '/usr/sbin/installer' '-pkg' '/Volumes/Toni'\\''s Disk/a.pkg'" with administrator privileges"#
        );
    }

    #[test]
    fn administrator_script_escapes_double_quotes_for_applescript_only() {
        let script = single_command_script(
            "/usr/sbin/installer",
            &["-pkg", r#"/Volumes/The "Best" Disk/a.pkg"#],
        );
        assert_eq!(
            script,
            r#"do shell script "cd '/' || exit 100; '/usr/sbin/installer' '-pkg' '/Volumes/The \"Best\" Disk/a.pkg'" with administrator privileges"#
        );
    }

    #[test]
    fn administrator_script_leaves_dollar_and_backticks_inside_single_quotes() {
        // Unquoted, each of these would run `id` or expand a variable as root.
        let script = single_command_script(
            "/bin/ls",
            &["/tmp/$HOME/a.pkg", "/tmp/`id`/b.pkg", "/tmp/$(id)/c.pkg"],
        );
        assert_eq!(
            script,
            r#"do shell script "cd '/' || exit 100; '/bin/ls' '/tmp/$HOME/a.pkg' '/tmp/`id`/b.pkg' '/tmp/$(id)/c.pkg'" with administrator privileges"#
        );
    }

    #[test]
    fn administrator_script_doubles_backslashes_for_applescript_only() {
        // A trailing `\` and a literal `\n` both stay plain characters: the
        // shell takes them as they are inside single quotes.
        let script = single_command_script("/bin/ls", &[r"/tmp/back\slash\", r"/tmp/\n"]);
        assert_eq!(
            script,
            r#"do shell script "cd '/' || exit 100; '/bin/ls' '/tmp/back\\slash\\' '/tmp/\\n'" with administrator privileges"#
        );
    }

    #[test]
    fn administrator_script_changes_to_the_given_working_directory() {
        let script = administrator_shell_script(
            Some(Path::new("/Volumes/Surge XT")),
            &[ShellCommand::new("/usr/bin/true", Vec::<String>::new())],
            ShellSteps::Unnumbered,
        )
        .unwrap();
        assert_eq!(
            script,
            r#"do shell script "cd '/Volumes/Surge XT' || exit 100; '/usr/bin/true'" with administrator privileges"#
        );
    }

    #[test]
    fn shell_lines_number_each_command_so_a_failure_names_its_step() {
        let script = administrator_shell_script(
            None,
            &[
                ShellCommand::new(
                    "/bin/rm",
                    ["-rf", "/Library/Audio/Plug-Ins/VST3/Surge XT.vst3"],
                ),
                ShellCommand::new(
                    "/usr/sbin/pkgutil",
                    ["--forget", "com.surge-synth-team.surge-xt.vst3.pkg"],
                ),
            ],
            ShellSteps::Numbered,
        )
        .unwrap();
        assert_eq!(
            script,
            r#"do shell script "cd '/' || exit 100; '/bin/rm' '-rf' '/Library/Audio/Plug-Ins/VST3/Surge XT.vst3' || exit 101; '/usr/sbin/pkgutil' '--forget' 'com.surge-synth-team.surge-xt.vst3.pkg' || exit 102" with administrator privileges"#
        );
    }

    /// Whatever an argument holds, the shell must end up with exactly that
    /// argument as one word. This checks both layers of quoting together.
    #[test]
    fn every_argument_reaches_the_program_unchanged() {
        let arguments = [
            "/Volumes/Surge XT/Surge XT/a.pkg",
            "it's",
            "''",
            r#"say "hi""#,
            "$HOME $(id) `id` ${PATH}",
            r"back\slash\",
            r"\n\t\\",
            "semi;colon && pipe | amp & star * tilde ~ hash #",
            "",
            "tab\tand ünïcödé",
        ];
        let script = administrator_shell_script(
            None,
            &[
                ShellCommand::new("/bin/echo", arguments),
                ShellCommand::new("/bin/rm", ["/Library/Audio/Plug-Ins/CLAP/Surge XT.clap"]),
            ],
            ShellSteps::Numbered,
        )
        .unwrap();

        let mut expected = vec!["cd", "/", "||", "exit", "100", ";", "/bin/echo"];
        expected.extend(arguments);
        expected.extend([
            "||",
            "exit",
            "101",
            ";",
            "/bin/rm",
            "/Library/Audio/Plug-Ins/CLAP/Surge XT.clap",
            "||",
            "exit",
            "102",
        ]);
        assert_eq!(words_the_shell_sees(&script), expected);
    }

    #[test]
    fn administrator_script_refuses_line_breaks_and_nul() {
        for argument in ["/tmp/a\nb", "/tmp/a\rb", "/tmp/a\0b"] {
            let result = administrator_shell_script(
                None,
                &[ShellCommand::new("/bin/rm", [argument])],
                ShellSteps::Numbered,
            );
            assert_eq!(
                refusal_reason(result),
                "an argument contains a line break or NUL character"
            );
        }

        let result = administrator_shell_script(
            None,
            &[ShellCommand::new("/tmp/odd\nprogram", Vec::<String>::new())],
            ShellSteps::Unnumbered,
        );
        assert_eq!(
            refusal_reason(result),
            "the program is not an absolute path the shell can take as it is"
        );
    }

    #[test]
    fn administrator_script_refuses_relative_programs_and_directories() {
        for program in ["installer", "usr/sbin/installer", ""] {
            let result = administrator_shell_script(
                None,
                &[ShellCommand::new(program, ["-pkg", "/tmp/a.pkg"])],
                ShellSteps::Unnumbered,
            );
            assert_eq!(
                refusal_reason(result),
                "the program is not an absolute path the shell can take as it is"
            );
        }

        let result = administrator_shell_script(
            Some(Path::new("Volumes/Surge XT")),
            &[ShellCommand::new("/usr/bin/true", Vec::<String>::new())],
            ShellSteps::Unnumbered,
        );
        assert_eq!(
            refusal_reason(result),
            "the working directory is not usable"
        );
    }

    #[test]
    fn shell_lines_stop_at_the_last_status_that_can_name_a_step() {
        let commands = vec![ShellCommand::new("/usr/bin/true", Vec::<String>::new()); 155];
        let script = administrator_shell_script(None, &commands, ShellSteps::Numbered).unwrap();
        assert!(script.ends_with(r#"|| exit 255" with administrator privileges"#));

        let commands = vec![ShellCommand::new("/usr/bin/true", Vec::<String>::new()); 156];
        let result = administrator_shell_script(None, &commands, ShellSteps::Numbered);
        assert_eq!(
            refusal_reason(result),
            "too many commands for one administrator prompt"
        );
    }

    #[test]
    fn osascript_errors_give_the_shell_status_or_a_cancel() {
        assert_eq!(
            administrator_shell_result(true, Some(0), ""),
            AdministratorShellResult::Exited(Some(0))
        );
        // osascript itself exits 1; the installer's status is the number at
        // the end of the error.
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:183: execution error: installer: Error - the package path specified was invalid. (1)\n"
            ),
            AdministratorShellResult::Exited(Some(1))
        );
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:57: execution error: The command exited with a non-zero status. (102)\n"
            ),
            AdministratorShellResult::Exited(Some(102))
        );
        // A message over several lines still ends with the number.
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:40: execution error: rm: first line\nrm: second line (3)\n"
            ),
            AdministratorShellResult::Exited(Some(3))
        );
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:28: execution error: User canceled. (-128)\n"
            ),
            AdministratorShellResult::Cancelled
        );
        // The message is translated on a system in another language; the
        // number is not.
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:28: execution error: Abbruch durch den Benutzer. (-128)\n"
            ),
            AdministratorShellResult::Cancelled
        );
        // The words alone count only when there is no number to go by.
        assert_eq!(
            administrator_shell_result(false, Some(1), "User canceled.\n"),
            AdministratorShellResult::Cancelled
        );
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:40: execution error: installer: User canceled the install (1)\n"
            ),
            AdministratorShellResult::Exited(Some(1))
        );
        // An AppleScript error is not a shell status, and neither is text
        // with no number: fall back to osascript's own exit status.
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:5: syntax error: Expected end of line. (-2741)\n"
            ),
            AdministratorShellResult::Exited(Some(1))
        );
        assert_eq!(
            administrator_shell_result(false, Some(1), "something went wrong\n"),
            AdministratorShellResult::Exited(Some(1))
        );
    }

    #[test]
    fn shell_lines_status_names_the_failed_step() {
        assert_eq!(
            shell_lines_outcome(Some(0), 2),
            ShellLinesOutcome::Completed
        );
        assert_eq!(
            shell_lines_outcome(Some(101), 2),
            ShellLinesOutcome::FailedAt { index: 0 }
        );
        assert_eq!(
            shell_lines_outcome(Some(102), 2),
            ShellLinesOutcome::FailedAt { index: 1 }
        );
        for code in [Some(100), Some(103), Some(1), Some(-1), None] {
            assert_eq!(
                shell_lines_outcome(code, 2),
                ShellLinesOutcome::Stopped { code }
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn shell_lines_are_macos_only() {
        let commands = [ShellCommand::new("/usr/bin/true", Vec::<String>::new())];
        assert!(matches!(
            super::run_elevated_shell_lines(&commands),
            Err(ElevationError::Unsupported)
        ));
    }

    /// The two values must map to different show commands, and `Hidden` must
    /// be the one that draws nothing. Getting this backwards is invisible in
    /// review and shows up only as a console window flashing on a user's
    /// screen mid-install, which is the bug this enum exists to fix.
    #[cfg(windows)]
    #[test]
    fn hidden_maps_to_the_show_command_that_draws_nothing() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};

        assert_eq!(ElevatedWindow::Hidden.show_command(), SW_HIDE);
        assert_eq!(ElevatedWindow::Show.show_command(), SW_SHOWNORMAL);
    }
}
