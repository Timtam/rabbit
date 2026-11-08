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
//! RABBIT doesn't have. Surge XT's `installer -pkg` flow uses this path,
//! and so does an uninstall's one administrator command
//! ([`run_administrator_shell`]).
//!
//! The shell behind `do shell script` splits words and expands `$`,
//! backticks and quotes, and it does so as root. So every word RABBIT
//! hands it is POSIX single-quoted first, and only then is the whole
//! command put into one AppleScript string. The text goes straight to
//! `osascript`; no script file is written, because a file in a temporary
//! folder could be rewritten by another process of the same user between
//! RABBIT writing it and root running it.
//!
//! Neither macOS path raises a prompt from a test build or from a program
//! that isn't named like RABBIT, so no test, here or in a crate that calls
//! these, can put a password window on a Mac or on CI.
//!
//! Other targets compile to a stub that returns an `Unsupported` error.

#[cfg(windows)]
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug)]
pub enum ElevationError {
    /// The program that raises the permission prompt couldn't be started:
    /// `ShellExecuteExW` on Windows, `osascript` on macOS. `code` is the
    /// OS error number, 0 when there is none.
    LaunchFailed { program: PathBuf, code: u32 },
    /// `WaitForSingleObject` or `GetExitCodeProcess` failed, or the process
    /// terminated abnormally so we have no exit code to report.
    WaitFailed { program: PathBuf, message: String },
    /// The user dismissed the permission prompt: UAC on Windows, the
    /// administrator password window on macOS. Distinct from a generic
    /// launch failure so the caller can surface a clearer message.
    UserCancelledElevation { program: PathBuf },
    /// macOS ended the administrator password window with an error rather
    /// than a cancel, so the command never ran: a wrong password three
    /// times, an account that isn't an administrator, no window to show it
    /// in (over SSH), or security software that blocks `osascript`.
    /// `number` is the AppleScript or Authorization error number, always
    /// negative.
    PromptFailed { number: i32 },
    /// RABBIT would not hand this command to a root shell, and nothing ran.
    /// `reason` says which rule it broke (see [`run_administrator_shell`]).
    RefusedCommand {
        program: PathBuf,
        reason: &'static str,
    },
    /// Compiled on a target that has no elevation primitive for this call:
    /// none at all outside Windows and macOS, and no administrator shell
    /// outside macOS.
    Unsupported,
}

impl std::fmt::Display for ElevationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LaunchFailed { program, code } => write!(
                f,
                "could not ask for administrator rights to run {} (system error {code})",
                program.display()
            ),
            Self::WaitFailed { program, message } => write!(
                f,
                "could not read exit status for elevated process {}: {message}",
                program.display()
            ),
            Self::UserCancelledElevation { program } => write!(
                f,
                "the administrator permission prompt for {} was cancelled or declined",
                program.display()
            ),
            Self::PromptFailed { number } => write!(
                f,
                "the administrator password prompt ended with error {number}, so nothing ran"
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
/// [`run_administrator_shell`] apply: an absolute program path, no line
/// breaks or NUL, every word passed exactly as given. It refuses, before
/// any prompt, in a test build and when the running program isn't a RABBIT
/// build by [`running_as_rabbit`].
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn run_elevated_and_wait(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    window: ElevatedWindow,
) -> Result<Option<i32>, ElevationError> {
    platform_run_elevated_and_wait(program, arguments, working_directory, window)
}

/// One command for a root shell: see [`ShellStep`] and
/// [`run_elevated_and_wait`].
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

/// One step of [`run_administrator_shell`]: a command, and what its exit
/// status means for the rest of the run.
///
/// Every step has its place in the list, counted from 0, and a failing
/// step that stops the run reports that place as
/// [`ShellLinesOutcome::FailedAt`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellStep {
    /// Any status but 0 stops the run at this step.
    Run(ShellCommand),
    /// Whatever the command does, the run goes on, and its error output is
    /// dropped. For a step whose failure changes nothing that matters,
    /// such as `pkgutil --forget` of a receipt that is already gone.
    RunIgnoringFailure(ShellCommand),
    /// Only status 1 lets the run go on. That is `pgrep`'s "nothing
    /// matched"; a match (0) and `pgrep`'s own errors (2 and 3) all stop
    /// the run at this step, so it fails closed.
    RunExpectingNoMatch(ShellCommand),
}

impl ShellStep {
    pub fn command(&self) -> &ShellCommand {
        match self {
            Self::Run(command)
            | Self::RunIgnoringFailure(command)
            | Self::RunExpectingNoMatch(command) => command,
        }
    }
}

/// The steps for one [`run_administrator_shell`] call: one password
/// window for all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdministratorShellRequest {
    pub steps: Vec<ShellStep>,
}

/// What [`run_administrator_shell`] can tell about a root shell that ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdministratorShellReport {
    pub outcome: ShellLinesOutcome,
    /// What the shell wrote to its error output when it didn't complete,
    /// usually the failing step's own message, such as `rm`'s "Operation
    /// not permitted". `osascript`'s position prefix and the status number
    /// it adds are removed, and the text is cut at 8 KB. `None` when the
    /// run completed or the shell wrote nothing to its error output.
    /// osascript's own "The command exited with a non-zero status." is
    /// dropped. Where macOS translates that sentence, the translation may
    /// still come through.
    pub error_text: Option<String>,
}

/// How far an administrator shell got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellLinesOutcome {
    /// Every step ran, and none stopped the run.
    Completed,
    /// The step at `index` (counting from 0, in the order given) stopped
    /// the run. Every step before it ran, and none after it did.
    FailedAt { index: usize },
    /// The shell ended without naming a step. Status 100 means it could
    /// not change to `/`, so nothing ran. With anything else, such as 255
    /// or a status RABBIT couldn't read (`None`), RABBIT can't tell which
    /// steps ran, and only the filesystem can say what changed.
    Stopped { code: Option<i32> },
}

/// Exit status of the root shell when it can't change to its working
/// directory. Nothing else has run at that point.
const SHELL_CD_FAILED_STATUS: i32 = 100;

/// Exit status when the first step of [`run_administrator_shell`] stops
/// the run; the step at index `n` exits with this plus `n`. Each command's
/// own status is replaced, so a program that happens to exit with 101
/// can't be mistaken for a step.
const SHELL_FIRST_STEP_STATUS: i32 = 101;

/// The one status no step uses. `do shell script` reports its own errors,
/// such as output it couldn't hold, as 255 (Apple's TN2065), so 255 is
/// read as "stopped, step unknown" and never as a step.
const SHELL_STOPPED_STATUS: i32 = 255;

/// Shell exit statuses end at 255, and 255 itself is reserved, which caps
/// how many steps one prompt can carry while still saying which one
/// failed: statuses 101 to 254.
const SHELL_MAX_STEPS: usize = (SHELL_STOPPED_STATUS - SHELL_FIRST_STEP_STATUS) as usize;

/// The most steps one [`run_administrator_shell`] call takes: 154. Callers
/// that build a request check against it before they announce a prompt,
/// so a request that would be refused never gets that far.
pub const ADMINISTRATOR_SHELL_MAX_STEPS: usize = SHELL_MAX_STEPS;

/// The program that raises the macOS administrator password window.
const OSASCRIPT: &str = "/usr/bin/osascript";

/// The most of the shell's error output [`AdministratorShellReport`]
/// keeps. `rm -R` prints a line for every file it couldn't remove, which
/// for a large bundle could run to megabytes; the first lines say enough.
const SHELL_ERROR_TEXT_LIMIT: usize = 8 * 1024;

/// What `do shell script` says for a shell that exits non-zero with
/// nothing on its error output. AppleScript's own words, not the step's.
const OSASCRIPT_NO_MESSAGE: &str = "The command exited with a non-zero status.";

/// Why a test build never raises an administrator prompt.
const TEST_BUILD_REFUSAL: &str = "a test build never asks for administrator rights";

/// Why a program that isn't RABBIT never raises one through here.
const NOT_RABBIT_REFUSAL: &str = "only RABBIT itself may ask for administrator rights this way, and this program isn't named like a RABBIT build";

/// Why RABBIT won't raise an administrator prompt from this program, or
/// `None` when it may. A test build never may, whatever its name, and
/// neither may a program that isn't named like RABBIT: other crates' test
/// binaries, which call into this crate without its `cfg(test)`, are
/// caught by the name.
fn administrator_prompt_refusal() -> Option<&'static str> {
    if cfg!(test) {
        Some(TEST_BUILD_REFUSAL)
    } else if !running_as_rabbit() {
        Some(NOT_RABBIT_REFUSAL)
    } else {
        None
    }
}

/// Run `request`'s steps one after another in a single root shell, behind
/// one administrator password window, and stop at the first step that
/// stops the run. This is how a macOS uninstall makes its one prompt.
///
/// The shell changes to `/`, sends its standard output to `/dev/null`, so
/// nothing a step prints can fill `do shell script`'s buffer and its error
/// message holds only error output, and then runs each step with every
/// word POSIX single-quoted. Steps are refused before anything runs if a
/// program path isn't absolute, if any word contains a line break or NUL,
/// or if there are more than 154 of them. Callers that remove files run
/// their own root guard on these exact strings first: this function only
/// makes sure the shell receives them unchanged.
///
/// The window shows macOS's own text; RABBIT adds none, and announces the
/// window itself before calling this.
///
/// The call blocks until the window is answered and the shell has ended.
/// If that takes longer than `still_waiting_after`, `on_still_waiting` is
/// called once, from this thread, while the wait goes on.
///
/// Cancelling the window is [`ElevationError::UserCancelledElevation`],
/// and any other failure of the window is [`ElevationError::PromptFailed`];
/// in both cases nothing ran, and neither did anything when `osascript`
/// couldn't start ([`ElevationError::LaunchFailed`]). Once it has started,
/// losing track of it is [`ElevationError::WaitFailed`]: the window may
/// have been answered and the steps run, so only the filesystem can say
/// what changed. A shell that ran gives an
/// [`AdministratorShellReport`]. An empty request returns
/// [`ShellLinesOutcome::Completed`] without a window.
///
/// It refuses, before any prompt, in a test build and when the running
/// program isn't a RABBIT build by [`running_as_rabbit`], so no test can
/// raise a real prompt through here. macOS only; other platforms return
/// `Unsupported`.
pub fn run_administrator_shell(
    request: &AdministratorShellRequest,
    still_waiting_after: Duration,
    on_still_waiting: &mut dyn FnMut(),
) -> Result<AdministratorShellReport, ElevationError> {
    if let Some(reason) = administrator_prompt_refusal() {
        return Err(ElevationError::RefusedCommand {
            program: PathBuf::from(OSASCRIPT),
            reason,
        });
    }
    let shell = administrator_shell_text(&request.steps)?;
    if request.steps.is_empty() {
        return Ok(AdministratorShellReport {
            outcome: ShellLinesOutcome::Completed,
            error_text: None,
        });
    }
    platform_run_administrator_shell(
        &shell,
        request.steps.len(),
        still_waiting_after,
        on_still_waiting,
    )
}

#[cfg(target_os = "macos")]
fn platform_run_administrator_shell(
    shell: &str,
    step_count: usize,
    still_waiting_after: Duration,
    on_still_waiting: &mut dyn FnMut(),
) -> Result<AdministratorShellReport, ElevationError> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let script = administrator_script(shell);
    let output = wait_with_notice(
        move || {
            let mut command = Command::new(OSASCRIPT);
            // `output()` would pipe these two by itself; `spawn()` inherits
            // all three unless told.
            command
                .arg("-e")
                .arg(&script)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            // A process group of its own: the first Ctrl+C in Terminal goes
            // to RABBIT's group only, so it can't end the password window or
            // root's steps halfway, and RABBIT reads the stop once osascript
            // has returned. The second Ctrl+C is passed on to this group
            // before RABBIT ends (see ctrl_c.rs), so a password window is
            // never left open with nobody waiting for its answer. osascript
            // never needs the terminal: its input is empty and its output
            // piped.
            command.process_group(0);
            // The root shell inherits this environment.
            crate::process::without_rabbit_secrets(&mut command);
            let child = command
                .spawn()
                .map_err(|err| ElevationError::LaunchFailed {
                    program: PathBuf::from(OSASCRIPT),
                    code: err.raw_os_error().unwrap_or(0) as u32,
                })?;
            // Its pid is its group's id, as process_group(0) made it.
            let _forward = i32::try_from(child.id())
                .ok()
                .map(crate::ctrl_c::forward_second_ctrl_c_to);
            // osascript has started: the window may have been answered and
            // root's steps run, so a failure from here on only means RABBIT
            // lost track of it, and the run must ask the filesystem.
            child
                .wait_with_output()
                .map_err(|err| ElevationError::WaitFailed {
                    program: PathBuf::from(OSASCRIPT),
                    message: err.to_string(),
                })
        },
        still_waiting_after,
        on_still_waiting,
    )
    .ok_or_else(|| ElevationError::WaitFailed {
        program: PathBuf::from(OSASCRIPT),
        message: "the thread waiting for osascript ended without an answer".to_string(),
    })??;

    administrator_shell_report(
        output.status.success(),
        output.status.code(),
        &String::from_utf8_lossy(&output.stderr),
        step_count,
    )
}

#[cfg(not(target_os = "macos"))]
fn platform_run_administrator_shell(
    _shell: &str,
    _step_count: usize,
    _still_waiting_after: Duration,
    _on_still_waiting: &mut dyn FnMut(),
) -> Result<AdministratorShellReport, ElevationError> {
    Err(ElevationError::Unsupported)
}

/// Run `work` on a thread of its own and wait for its answer. If it takes
/// longer than `still_waiting_after`, call `on_still_waiting` once, here,
/// and keep waiting. `None` when the thread can't be started or ends
/// without answering.
///
/// The password window can stay open for as long as the user leaves it,
/// and `osascript` has no way to report that it is still waiting; this is
/// what lets RABBIT say so.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn wait_with_notice<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    still_waiting_after: Duration,
    on_still_waiting: &mut dyn FnMut(),
) -> Option<T> {
    use std::sync::mpsc::{RecvTimeoutError, channel};

    let (sender, receiver) = channel();
    std::thread::Builder::new()
        .name("rabbit-administrator-shell".to_string())
        .spawn(move || {
            // The receiver only goes away if the waiting side has given up,
            // and then nobody needs the answer.
            let _ = sender.send(work());
        })
        .ok()?;
    match receiver.recv_timeout(still_waiting_after) {
        Ok(answer) => Some(answer),
        Err(RecvTimeoutError::Timeout) => {
            on_still_waiting();
            receiver.recv().ok()
        }
        Err(RecvTimeoutError::Disconnected) => None,
    }
}

/// A program [`spawn_elevated`] started with administrator rights.
///
/// Dropping it closes RABBIT's handle to the process and leaves the process
/// running: a non-elevated RABBIT can't end an elevated process anyway.
#[derive(Debug)]
pub struct ElevatedChild {
    #[cfg(windows)]
    process: std::os::windows::io::OwnedHandle,
    /// Only Windows starts one, so elsewhere there is never a value.
    #[cfg(not(windows))]
    never: std::convert::Infallible,
    pid: u32,
    program: PathBuf,
}

impl ElevatedChild {
    /// The process ID, which the elevated helper's pipe checks its client
    /// against.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The exit code if the process has ended, without waiting.
    pub fn try_wait(&self) -> Result<Option<i32>, ElevationError> {
        self.wait_timeout(std::time::Duration::ZERO)
    }

    /// Wait up to `timeout` for the process to end. `Some(exit code)` once it
    /// has, `None` when it is still running. Timeouts beyond 49 days are cut
    /// to that.
    pub fn wait_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Option<i32>, ElevationError> {
        #[cfg(windows)]
        {
            // INFINITE is u32::MAX, so a finite wait stops one below it.
            let milliseconds = u32::try_from(timeout.as_millis())
                .unwrap_or(u32::MAX - 1)
                .min(u32::MAX - 1);
            wait_for_exit(&self.process, milliseconds, &self.program)
        }
        #[cfg(not(windows))]
        {
            let _ = timeout;
            match self.never {}
        }
    }
}

/// Start `program` with administrator rights and return at once, without
/// waiting for it to end. Windows only; elsewhere `Unsupported`.
///
/// This raises the Windows permission (UAC) prompt and returns once the
/// user has answered it. Declining is
/// [`ElevationError::UserCancelledElevation`]. The arguments are quoted as
/// for [`run_elevated_and_wait`]; `window` works the same way.
///
/// It refuses, before any prompt, when `program` isn't an absolute path, or
/// when the running program isn't a RABBIT build by [`running_as_rabbit`]:
/// test binaries never are, so no test can raise a real prompt through
/// here, even by mistake.
pub fn spawn_elevated(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    window: ElevatedWindow,
) -> Result<ElevatedChild, ElevationError> {
    if !running_as_rabbit() {
        return Err(ElevationError::RefusedCommand {
            program: program.to_path_buf(),
            reason: NOT_RABBIT_REFUSAL,
        });
    }
    if !program.is_absolute() {
        return Err(ElevationError::RefusedCommand {
            program: program.to_path_buf(),
            reason: "the program's path isn't absolute",
        });
    }
    platform_spawn_elevated(program, arguments, working_directory, window)
}

#[cfg(windows)]
fn platform_spawn_elevated(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    window: ElevatedWindow,
) -> Result<ElevatedChild, ElevationError> {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::System::Threading::GetProcessId;
    use windows_sys::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS};

    // NOASYNC: the calling thread may be a worker without a message loop,
    // and the launch must be complete before this returns.
    let process = shell_execute_runas(
        program,
        arguments,
        working_directory,
        window,
        SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
    )?;
    // SAFETY: `process` is a live process handle owned by this function.
    let pid = unsafe { GetProcessId(process.as_raw_handle()) };
    if pid == 0 {
        return Err(ElevationError::WaitFailed {
            program: program.to_path_buf(),
            message: "could not read the elevated process's ID".to_string(),
        });
    }
    Ok(ElevatedChild {
        process,
        pid,
        program: program.to_path_buf(),
    })
}

#[cfg(not(windows))]
fn platform_spawn_elevated(
    _program: &Path,
    _arguments: &[String],
    _working_directory: Option<&Path>,
    _window: ElevatedWindow,
) -> Result<ElevatedChild, ElevationError> {
    Err(ElevationError::Unsupported)
}

/// Whether the running program is a RABBIT build, judged by its file name
/// with [`is_rabbit_file_stem`].
pub fn running_as_rabbit() -> bool {
    std::env::current_exe()
        .ok()
        .as_deref()
        .is_some_and(is_rabbit_program)
}

/// Whether `program`, a path to a program file, is named like a RABBIT
/// build. On macOS `current_exe()` gives the path the program was started
/// by, unresolved: `.../Rabbit.app/Contents/MacOS/rabbit` for the app
/// (`CFBundleExecutable` in packaging/macos/Info.plist), even when macOS
/// runs it from a translocated copy, and the download's own name for the
/// bare binary.
fn is_rabbit_program(program: &Path) -> bool {
    program
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(is_rabbit_file_stem)
}

/// Whether a program file stem (its name without `.exe`) is one a RABBIT
/// build carries, ignoring case: `rabbit` as cargo builds it and as the
/// macOS app bundle runs it, `rabbit-windows-x86_64` and
/// `rabbit-0.5.0-windows-aarch64` as releases publish it, the macOS
/// binaries `rabbit-macos-universal` and `rabbit-0.5.0-macos-universal`
/// (whose stem, cut at the last dot, is `rabbit-0.5`), and any of these
/// with more after a space or a dash, such as a browser's ` (1)`.
///
/// Never a cargo test binary, which is named `<crate>-<16 hex digits>`
/// (`rabbit-0123456789abcdef` for the `rabbit` crate's own tests,
/// `rabbit_core-...` for the library's).
pub fn is_rabbit_file_stem(stem: &str) -> bool {
    let stem = stem.to_ascii_lowercase();
    let Some(rest) = stem.strip_prefix("rabbit") else {
        return false;
    };
    let named_like_rabbit = rest.is_empty() || rest.starts_with(['-', ' ', '(']);
    named_like_rabbit && !is_cargo_test_binary_stem(&stem)
}

fn is_cargo_test_binary_stem(stem: &str) -> bool {
    stem.rsplit_once('-').is_some_and(|(_, hash)| {
        hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

/// Whether this process runs with administrator rights: elevated through
/// UAC, or started by an account whose rights UAC doesn't split. Windows
/// only; elsewhere `Unsupported`.
pub fn is_current_process_elevated() -> std::io::Result<bool> {
    platform_is_current_process_elevated()
}

#[cfg(windows)]
fn platform_is_current_process_elevated() -> std::io::Result<bool> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

    use windows_sys::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut raw_token = std::ptr::null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo handle that needs no
    // closing, and `raw_token` is a valid place for the token handle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: the call succeeded, so the handle is ours to close.
    let token = unsafe { OwnedHandle::from_raw_handle(raw_token) };
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    // SAFETY: `elevation` is a TOKEN_ELEVATION and the size passed is its
    // size, so Windows writes only inside it.
    let ok = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenElevation,
            (&raw mut elevation).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(elevation.TokenIsElevated != 0)
}

#[cfg(not(windows))]
fn platform_is_current_process_elevated() -> std::io::Result<bool> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "only Windows has UAC elevation",
    ))
}

/// Wait up to `milliseconds` (or `INFINITE`) for `process` to end: its exit
/// code once it has, `None` while it runs.
#[cfg(windows)]
fn wait_for_exit(
    process: &std::os::windows::io::OwnedHandle,
    milliseconds: u32,
    program: &Path,
) -> Result<Option<i32>, ElevationError> {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Foundation::{GetLastError, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};

    // SAFETY: `process` is a live process handle for the whole call.
    let wait_status = unsafe { WaitForSingleObject(process.as_raw_handle(), milliseconds) };
    if wait_status == WAIT_TIMEOUT {
        return Ok(None);
    }
    if wait_status != WAIT_OBJECT_0 {
        // SAFETY: no other call sits between the wait and this.
        let code = unsafe { GetLastError() };
        return Err(ElevationError::WaitFailed {
            program: program.to_path_buf(),
            message: format!("WaitForSingleObject failed (Win32 error {code})"),
        });
    }

    let mut exit_code: u32 = 0;
    // SAFETY: as above, and `exit_code` is a valid place for the code.
    let ok = unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut exit_code) };
    if ok == 0 {
        // SAFETY: no other call sits between the failure and this.
        let code = unsafe { GetLastError() };
        return Err(ElevationError::WaitFailed {
            program: program.to_path_buf(),
            message: format!("GetExitCodeProcess failed (Win32 error {code})"),
        });
    }
    Ok(Some(exit_code as i32))
}

#[cfg(windows)]
fn platform_run_elevated_and_wait(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    window: ElevatedWindow,
) -> Result<Option<i32>, ElevationError> {
    use windows_sys::Win32::System::Threading::INFINITE;
    use windows_sys::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;

    let process = shell_execute_runas(
        program,
        arguments,
        working_directory,
        window,
        SEE_MASK_NOCLOSEPROCESS,
    )?;
    wait_for_exit(&process, INFINITE, program)
}

/// Start `program` through `ShellExecuteExW` with the `runas` verb, which
/// raises the permission prompt when RABBIT isn't elevated, and return the
/// process handle. `mask` must include `SEE_MASK_NOCLOSEPROCESS`.
#[cfg(windows)]
fn shell_execute_runas(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    window: ElevatedWindow,
    mask: u32,
) -> Result<std::os::windows::io::OwnedHandle, ElevationError> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};

    use windows_sys::Win32::Foundation::{ERROR_CANCELLED, GetLastError};
    use windows_sys::Win32::UI::Shell::{SHELLEXECUTEINFOW, ShellExecuteExW};

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
        fMask: mask,
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

    // SAFETY: with SEE_MASK_NOCLOSEPROCESS the handle is the caller's to
    // close, and nothing else holds it.
    Ok(unsafe { OwnedHandle::from_raw_handle(info.hProcess) })
}

#[cfg(target_os = "macos")]
fn platform_run_elevated_and_wait(
    program: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    window: ElevatedWindow,
) -> Result<Option<i32>, ElevationError> {
    // Checked before anything else, so a test can't get past it even with
    // a command the rules below would refuse.
    if let Some(reason) = administrator_prompt_refusal() {
        return Err(ElevationError::RefusedCommand {
            program: program.to_path_buf(),
            reason,
        });
    }
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
    let script = administrator_script(&single_command_shell_text(working_directory, &command)?);
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

    let mut command = Command::new(OSASCRIPT);
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
        // The installer never started, so its status would be a guess.
        AdministratorShellResult::PromptFailed { number } => {
            Err(ElevationError::PromptFailed { number })
        }
        AdministratorShellResult::ShellExited(status) => Ok(Some(status)),
        AdministratorShellResult::Unclear(code) => Ok(code),
    }
}

/// The start of every root shell: change to `working_directory`, or `/`
/// when there is none, so no command depends on whatever folder RABBIT
/// itself was started from. `first_program` only names the command in a
/// refusal.
fn shell_prologue(
    working_directory: Option<&Path>,
    first_program: &Path,
) -> Result<String, ElevationError> {
    let directory = match working_directory {
        Some(directory) => {
            absolute_shell_path(directory).ok_or_else(|| ElevationError::RefusedCommand {
                program: first_program.to_path_buf(),
                reason: "the working directory is not usable",
            })?
        }
        None => "/",
    };
    Ok(format!(
        "cd {} || exit {SHELL_CD_FAILED_STATUS}",
        posix_quote(directory)
    ))
}

/// `command` as one line of shell text, every word POSIX single-quoted,
/// which the shell takes literally whatever it holds.
fn command_text(command: &ShellCommand) -> Result<String, ElevationError> {
    let refuse = |reason: &'static str| ElevationError::RefusedCommand {
        program: command.program.clone(),
        reason,
    };
    let program = absolute_shell_path(&command.program)
        .ok_or_else(|| refuse("the program is not an absolute path the shell can take as it is"))?;
    if command
        .arguments
        .iter()
        .any(|argument| !is_shell_safe_word(argument))
    {
        return Err(refuse("an argument contains a line break or NUL character"));
    }
    let mut text = posix_quote(program);
    for argument in &command.arguments {
        text.push(' ');
        text.push_str(&posix_quote(argument));
    }
    Ok(text)
}

/// The root shell text for [`run_elevated_and_wait`]: one command, whose
/// own exit status is the shell's, which is what callers report.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn single_command_shell_text(
    working_directory: Option<&Path>,
    command: &ShellCommand,
) -> Result<String, ElevationError> {
    Ok(format!(
        "{}; {}",
        shell_prologue(working_directory, &command.program)?,
        command_text(command)?
    ))
}

/// The exact `/bin/sh` text [`run_administrator_shell`] hands to root for
/// `steps`, or the refusal it would give. Pure, so tests on every platform
/// can check what root receives, and tests on macOS can run the same text
/// through an unprivileged `/bin/sh`.
///
/// The shell changes to `/` (status 100 if it can't), sends its standard
/// output to `/dev/null`, then runs the steps in order, each followed by
/// the fixed text for its kind (`n` is 101 plus the step's index):
///
/// - [`ShellStep::Run`]: `<command> || exit <n>`
/// - [`ShellStep::RunIgnoringFailure`]: `<command> 2>/dev/null || :`
/// - [`ShellStep::RunExpectingNoMatch`]: `<command>; [ "$?" -eq 1 ] || exit <n>`
///
/// Only POSIX constructs, because `/bin/sh` may be bash, dash or zsh. The
/// fixed text is RABBIT's own; nothing from a manifest or a path is ever
/// left unquoted. More than 154 steps are refused, because statuses 101 to
/// 254 are all a shell can use to name a step.
pub fn administrator_shell_text(steps: &[ShellStep]) -> Result<String, ElevationError> {
    let first_program = steps
        .first()
        .map(|step| step.command().program.clone())
        .unwrap_or_else(|| PathBuf::from(OSASCRIPT));
    if steps.len() > SHELL_MAX_STEPS {
        return Err(ElevationError::RefusedCommand {
            program: first_program,
            reason: "too many commands for one administrator prompt",
        });
    }

    let mut shell = shell_prologue(None, &first_program)?;
    // Nothing a step prints reaches `do shell script`, which keeps its
    // buffer from overrunning (reported as 255) and leaves only error
    // output in the message RABBIT reads back.
    shell.push_str("; exec >/dev/null");
    for (index, step) in steps.iter().enumerate() {
        // `index` is capped by SHELL_MAX_STEPS above, so this fits below 255.
        let status = SHELL_FIRST_STEP_STATUS + index as i32;
        let command = command_text(step.command())?;
        shell.push_str("; ");
        shell.push_str(&command);
        match step {
            ShellStep::Run(_) => {
                let _ = write!(shell, " || exit {status}");
            }
            ShellStep::RunIgnoringFailure(_) => shell.push_str(" 2>/dev/null || :"),
            ShellStep::RunExpectingNoMatch(_) => {
                let _ = write!(shell, "; [ \"$?\" -eq 1 ] || exit {status}");
            }
        }
    }
    Ok(shell)
}

/// The AppleScript that runs `shell` as root: the whole shell text escaped
/// once as an AppleScript string. The password window keeps macOS's own
/// text; RABBIT adds no `with prompt`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn administrator_script(shell: &str) -> String {
    format!(
        "do shell script {} with administrator privileges",
        applescript_quote(shell)
    )
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
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum AdministratorShellResult {
    /// The user dismissed the administrator prompt; nothing ran.
    Cancelled,
    /// The prompt failed with this AppleScript or Authorization error,
    /// which is always negative. A shell's status never is, so nothing ran.
    PromptFailed { number: i32 },
    /// The shell ran and exited with this status, from 1 to 255.
    ShellExited(i32),
    /// `osascript` failed without a number that says what happened:
    /// `osascript`'s own status, `None` when a signal ended it. Whether
    /// the shell ran is unknown.
    Unclear(Option<i32>),
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
        return AdministratorShellResult::ShellExited(0);
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
        Some(status @ 1..=255) => AdministratorShellResult::ShellExited(status),
        // A wrong password three times, an account that isn't an
        // administrator, no window server, a script error: AppleScript and
        // Authorization Services number their errors below zero.
        Some(number) if number < 0 => AdministratorShellResult::PromptFailed { number },
        _ => AdministratorShellResult::Unclear(osascript_status),
    }
}

/// The number at the end of an `osascript` error: the `-128` in
/// `0:28: execution error: User canceled. (-128)`.
fn applescript_error_number(stderr: &str) -> Option<i32> {
    let inner = stderr.trim_end().strip_suffix(')')?;
    let (_, number) = inner.rsplit_once('(')?;
    number.parse().ok()
}

/// Everything [`run_administrator_shell`] reads from a finished
/// `osascript`: a cancel or a failed prompt as the error they are, or how
/// far a shell of `step_count` steps got and what it said. Pure, so every
/// case can be tested on every platform.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn administrator_shell_report(
    success: bool,
    osascript_status: Option<i32>,
    stderr: &str,
    step_count: usize,
) -> Result<AdministratorShellReport, ElevationError> {
    let outcome = match administrator_shell_result(success, osascript_status, stderr) {
        AdministratorShellResult::Cancelled => {
            return Err(ElevationError::UserCancelledElevation {
                program: PathBuf::from(OSASCRIPT),
            });
        }
        AdministratorShellResult::PromptFailed { number } => {
            return Err(ElevationError::PromptFailed { number });
        }
        AdministratorShellResult::ShellExited(status) => shell_steps_outcome(status, step_count),
        // Not even a status to go by: osascript's own 1 says nothing about
        // which steps ran.
        AdministratorShellResult::Unclear(_) => ShellLinesOutcome::Stopped { code: None },
    };
    let error_text = match outcome {
        ShellLinesOutcome::Completed => None,
        _ => shell_error_text(stderr),
    };
    Ok(AdministratorShellReport {
        outcome,
        error_text,
    })
}

/// Which step a shell status of a run of `step_count` steps names.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn shell_steps_outcome(status: i32, step_count: usize) -> ShellLinesOutcome {
    if status == 0 {
        return ShellLinesOutcome::Completed;
    }
    let step = (SHELL_FIRST_STEP_STATUS..SHELL_STOPPED_STATUS)
        .contains(&status)
        .then(|| (status - SHELL_FIRST_STEP_STATUS) as usize)
        .filter(|index| *index < step_count);
    match step {
        Some(index) => ShellLinesOutcome::FailedAt { index },
        None => ShellLinesOutcome::Stopped { code: Some(status) },
    }
}

/// The shell's error output as `osascript` printed it, without the
/// `0:123: execution error: ` it puts in front and the ` (102)` it puts
/// after, with line ends made plain and cut at 8 KB on a character
/// boundary. `None` when nothing is left. `None` also for osascript's own
/// sentence for a step that wrote nothing.
///
/// `osascript`'s own labels are English on every system as far as RABBIT
/// knows; if one ever isn't, the prefix stays in the text, which loses
/// nothing.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn shell_error_text(stderr: &str) -> Option<String> {
    // `do shell script` turns the shell's line feeds into returns.
    let plain = stderr.replace("\r\n", "\n").replace('\r', "\n");
    let mut text = plain.trim();
    if let Some((before, number)) = text
        .strip_suffix(')')
        .and_then(|inner| inner.rsplit_once('('))
        && number.parse::<i32>().is_ok()
    {
        text = before.trim_end();
    }
    text = strip_osascript_position(text).trim();
    if text.is_empty() || text == OSASCRIPT_NO_MESSAGE {
        return None;
    }
    let mut end = text.len().min(SHELL_ERROR_TEXT_LIMIT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Some(text[..end].to_string())
}

/// `text` from `osascript`'s error line on, without the
/// `<start>:<end>: execution error: ` it starts with, or without just the
/// `<start>:<end>: ` when the label differs. The first line that starts
/// that way is `osascript`'s: its error line always comes before the
/// message it carries, so a later line of the message that happens to look
/// like a position stays in it. Lines before it can only be log output of
/// `osascript`'s own, which starts with a date, and are dropped. `text` as
/// it is when no line starts that way.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn strip_osascript_position(text: &str) -> &str {
    let mut line_starts =
        std::iter::once(0).chain(text.match_indices('\n').map(|(index, _)| index + 1));
    let Some(rest) = line_starts.find_map(|start| after_osascript_position(&text[start..])) else {
        return text;
    };
    rest.strip_prefix("execution error:")
        .map(str::trim_start)
        .unwrap_or(rest)
}

/// `line` after a leading `<start>:<end>: `, the place in the script an
/// `osascript` error names, when it starts with one.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn after_osascript_position(line: &str) -> Option<&str> {
    let is_number = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let (start, rest) = line.split_once(':')?;
    let (end, rest) = rest.split_once(": ")?;
    (is_number(start) && is_number(end)).then_some(rest)
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
    use super::{
        AdministratorShellReport, AdministratorShellRequest, AdministratorShellResult,
        ElevatedWindow, ElevationError, NOT_RABBIT_REFUSAL, SHELL_ERROR_TEXT_LIMIT, ShellCommand,
        ShellLinesOutcome, ShellStep, TEST_BUILD_REFUSAL, administrator_script,
        administrator_shell_report, administrator_shell_result, administrator_shell_text,
        posix_quote, quote_arguments, run_administrator_shell, shell_error_text,
        single_command_shell_text, wait_with_notice,
    };
    use std::path::Path;
    use std::time::Duration;

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
        administrator_script(
            &single_command_shell_text(
                None,
                &ShellCommand::new(program, arguments.iter().copied()),
            )
            .unwrap(),
        )
    }

    fn refusal_reason(result: Result<String, ElevationError>) -> &'static str {
        match result {
            Err(ElevationError::RefusedCommand { reason, .. }) => reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    fn run(program: &str, arguments: &[&str]) -> ShellStep {
        ShellStep::Run(ShellCommand::new(program, arguments.iter().copied()))
    }

    fn tolerated(program: &str, arguments: &[&str]) -> ShellStep {
        ShellStep::RunIgnoringFailure(ShellCommand::new(program, arguments.iter().copied()))
    }

    fn expecting_no_match(program: &str, arguments: &[&str]) -> ShellStep {
        ShellStep::RunExpectingNoMatch(ShellCommand::new(program, arguments.iter().copied()))
    }

    /// Undo the AppleScript layer of an administrator script: the frame and
    /// its escaping. A bare `"` would have ended the AppleScript string
    /// early, so it fails the test.
    fn shell_text_of(script: &str) -> String {
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
        shell
    }

    /// RABBIT's own shell syntax, the only text outside single quotes that
    /// isn't a plain word. Each is taken as one token.
    const FIXED_SHELL_SYNTAX: [&str; 4] =
        ["exec >/dev/null", "2>/dev/null", "[ \"$?\" -eq 1 ]", ":"];

    /// Split shell text into words the way `/bin/sh` would. Only the little
    /// RABBIT ever leaves outside single quotes is accepted there: letters,
    /// digits, `|`, `;` and [`FIXED_SHELL_SYNTAX`]. A `$`, backtick, `"`,
    /// `>` or blank that escaped the quoting fails the test instead of
    /// being guessed at.
    fn words_the_shell_sees(shell: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut word: Option<String> = None;
        let mut rest = shell;
        while let Some(ch) = rest.chars().next() {
            if word.is_none()
                && let Some(fixed) = FIXED_SHELL_SYNTAX.iter().find(|fixed| {
                    rest.strip_prefix(**fixed)
                        .is_some_and(|after| after.is_empty() || after.starts_with([' ', ';']))
                })
            {
                words.push(fixed.to_string());
                rest = &rest[fixed.len()..];
                continue;
            }
            rest = &rest[ch.len_utf8()..];
            match ch {
                ' ' => words.extend(word.take()),
                ';' => {
                    words.extend(word.take());
                    words.push(";".to_string());
                }
                '\'' => {
                    let word = word.get_or_insert_with(String::new);
                    let closing = rest
                        .find('\'')
                        .unwrap_or_else(|| panic!("single quote is never closed in {shell}"));
                    word.push_str(&rest[..closing]);
                    rest = &rest[closing + 1..];
                }
                '\\' => {
                    let escaped = rest.chars().next().expect("shell escape is complete");
                    word.get_or_insert_with(String::new).push(escaped);
                    rest = &rest[escaped.len_utf8()..];
                }
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
        let shell = single_command_shell_text(
            Some(Path::new("/Volumes/Surge XT")),
            &ShellCommand::new("/usr/bin/true", Vec::<String>::new()),
        )
        .unwrap();
        assert_eq!(shell, "cd '/Volumes/Surge XT' || exit 100; '/usr/bin/true'");
    }

    /// The three step kinds, as the uninstall uses them: the check that no
    /// blocking program started, a removal, and a receipt that may already
    /// be gone.
    #[test]
    fn each_step_kind_gets_its_own_fixed_shell_text() {
        let steps = [
            expecting_no_match("/usr/bin/pgrep", &["-x", "-i", "Surge XT|Surge XT Effects"]),
            run("/bin/rm", &["-R", "-f", "-x", "/Applications/Surge XT.app"]),
            tolerated(
                "/usr/sbin/pkgutil",
                &["--forget", "org.surge-synth-team.surge-xt.app.pkg"],
            ),
        ];
        let shell = administrator_shell_text(&steps).unwrap();
        assert_eq!(
            shell,
            r#"cd '/' || exit 100; exec >/dev/null; '/usr/bin/pgrep' '-x' '-i' 'Surge XT|Surge XT Effects'; [ "$?" -eq 1 ] || exit 101; '/bin/rm' '-R' '-f' '-x' '/Applications/Surge XT.app' || exit 102; '/usr/sbin/pkgutil' '--forget' 'org.surge-synth-team.surge-xt.app.pkg' 2>/dev/null || :"#
        );
        // The AppleScript layer escapes only the double quotes of the fixed
        // `"$?"`; nothing else in the text needs it.
        assert_eq!(
            administrator_script(&shell),
            r#"do shell script "cd '/' || exit 100; exec >/dev/null; '/usr/bin/pgrep' '-x' '-i' 'Surge XT|Surge XT Effects'; [ \"$?\" -eq 1 ] || exit 101; '/bin/rm' '-R' '-f' '-x' '/Applications/Surge XT.app' || exit 102; '/usr/sbin/pkgutil' '--forget' 'org.surge-synth-team.surge-xt.app.pkg' 2>/dev/null || :" with administrator privileges"#
        );
        assert_eq!(shell_text_of(&administrator_script(&shell)), shell);
    }

    #[test]
    fn numbered_steps_name_their_place_in_the_list_when_they_fail() {
        let shell = administrator_shell_text(&[
            run(
                "/bin/rm",
                &["-rf", "/Library/Audio/Plug-Ins/VST3/Surge XT.vst3"],
            ),
            run(
                "/usr/sbin/pkgutil",
                &["--forget", "org.surge-synth-team.surge-xt.vst3.pkg"],
            ),
        ])
        .unwrap();
        assert_eq!(
            shell,
            "cd '/' || exit 100; exec >/dev/null; '/bin/rm' '-rf' '/Library/Audio/Plug-Ins/VST3/Surge XT.vst3' || exit 101; '/usr/sbin/pkgutil' '--forget' 'org.surge-synth-team.surge-xt.vst3.pkg' || exit 102"
        );
    }

    /// A tolerated step keeps its place in the list, so the step after it
    /// still names its own index.
    #[test]
    fn a_tolerated_step_keeps_its_place_in_the_numbering() {
        let shell = administrator_shell_text(&[
            tolerated("/usr/bin/false", &[]),
            run("/usr/bin/true", &[]),
        ])
        .unwrap();
        assert!(
            shell.ends_with("'/usr/bin/false' 2>/dev/null || :; '/usr/bin/true' || exit 102"),
            "{shell}"
        );
    }

    #[test]
    fn an_empty_list_only_changes_to_root_and_silences_output() {
        assert_eq!(
            administrator_shell_text(&[]).unwrap(),
            "cd '/' || exit 100; exec >/dev/null"
        );
    }

    /// Whatever an argument holds, the shell must end up with exactly that
    /// argument as one word. This checks both layers of quoting together,
    /// with every step kind's fixed text around the words.
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
            "semi;colon && pipe | amp & star * tilde ~ hash # gt > lt <",
            "",
            "tab\tand ünïcödé",
            "[ \"$?\" -eq 1 ] || :",
        ];
        let shell = administrator_shell_text(&[
            ShellStep::Run(ShellCommand::new("/bin/echo", arguments)),
            tolerated("/bin/rm", &["/Library/Audio/Plug-Ins/CLAP/Surge XT.clap"]),
            expecting_no_match("/usr/bin/pgrep", &["-x", "-i", "Surge XT|REAPER"]),
        ])
        .unwrap();
        let script = administrator_script(&shell);

        let mut expected = vec![
            "cd",
            "/",
            "||",
            "exit",
            "100",
            ";",
            "exec >/dev/null",
            ";",
            "/bin/echo",
        ];
        expected.extend(arguments);
        expected.extend([
            "||",
            "exit",
            "101",
            ";",
            "/bin/rm",
            "/Library/Audio/Plug-Ins/CLAP/Surge XT.clap",
            "2>/dev/null",
            "||",
            ":",
            ";",
            "/usr/bin/pgrep",
            "-x",
            "-i",
            "Surge XT|REAPER",
            ";",
            "[ \"$?\" -eq 1 ]",
            "||",
            "exit",
            "103",
        ]);
        assert_eq!(words_the_shell_sees(&shell_text_of(&script)), expected);
    }

    #[test]
    fn line_breaks_and_nul_are_refused_in_every_step_kind() {
        for argument in ["/tmp/a\nb", "/tmp/a\rb", "/tmp/a\0b"] {
            for step in [
                run("/bin/rm", &[argument]),
                tolerated("/bin/rm", &[argument]),
                expecting_no_match("/usr/bin/pgrep", &[argument]),
            ] {
                assert_eq!(
                    refusal_reason(administrator_shell_text(&[step])),
                    "an argument contains a line break or NUL character"
                );
            }
        }

        let result = single_command_shell_text(
            None,
            &ShellCommand::new("/tmp/odd\nprogram", Vec::<String>::new()),
        );
        assert_eq!(
            refusal_reason(result),
            "the program is not an absolute path the shell can take as it is"
        );
    }

    #[test]
    fn relative_programs_and_directories_are_refused() {
        for program in ["installer", "usr/sbin/installer", ""] {
            let result = single_command_shell_text(
                None,
                &ShellCommand::new(program, ["-pkg", "/tmp/a.pkg"]),
            );
            assert_eq!(
                refusal_reason(result),
                "the program is not an absolute path the shell can take as it is"
            );
            assert_eq!(
                refusal_reason(administrator_shell_text(&[run(program, &[])])),
                "the program is not an absolute path the shell can take as it is"
            );
        }

        let result = single_command_shell_text(
            Some(Path::new("Volumes/Surge XT")),
            &ShellCommand::new("/usr/bin/true", Vec::<String>::new()),
        );
        assert_eq!(
            refusal_reason(result),
            "the working directory is not usable"
        );
    }

    /// 255 is left for `do shell script`'s own errors, so the last step a
    /// prompt can carry exits with 254.
    #[test]
    fn one_prompt_carries_at_most_154_steps() {
        let steps = vec![run("/usr/bin/true", &[]); 154];
        let shell = administrator_shell_text(&steps).unwrap();
        assert!(shell.ends_with("|| exit 254"), "{shell}");
        assert!(!shell.contains("exit 255"));

        let steps = vec![run("/usr/bin/true", &[]); 155];
        assert_eq!(
            refusal_reason(administrator_shell_text(&steps)),
            "too many commands for one administrator prompt"
        );
    }

    #[test]
    fn osascript_errors_give_the_shell_status_a_cancel_or_a_failed_prompt() {
        assert_eq!(
            administrator_shell_result(true, Some(0), ""),
            AdministratorShellResult::ShellExited(0)
        );
        // osascript itself exits 1; the installer's status is the number at
        // the end of the error.
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:183: execution error: installer: Error - the package path specified was invalid. (1)\n"
            ),
            AdministratorShellResult::ShellExited(1)
        );
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:57: execution error: The command exited with a non-zero status. (102)\n"
            ),
            AdministratorShellResult::ShellExited(102)
        );
        // A message over several lines still ends with the number.
        assert_eq!(
            administrator_shell_result(
                false,
                Some(1),
                "0:40: execution error: rm: first line\nrm: second line (3)\n"
            ),
            AdministratorShellResult::ShellExited(3)
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
            AdministratorShellResult::ShellExited(1)
        );
        // A negative number is the prompt's or the script's own error,
        // never a shell status: nothing ran.
        for (stderr, number) in [
            ("0:5: syntax error: Expected end of line. (-2741)\n", -2741),
            (
                "0:120: execution error: The administrator user name or password was incorrect. (-60007)\n",
                -60007,
            ),
            (
                "0:120: execution error: The authorization was denied. (-60005)\n",
                -60005,
            ),
        ] {
            assert_eq!(
                administrator_shell_result(false, Some(1), stderr),
                AdministratorShellResult::PromptFailed { number },
                "{stderr}"
            );
        }
        // Text with no number says nothing about the shell: osascript's
        // own status, or none when a signal ended it.
        assert_eq!(
            administrator_shell_result(false, Some(1), "something went wrong\n"),
            AdministratorShellResult::Unclear(Some(1))
        );
        assert_eq!(
            administrator_shell_result(false, None, ""),
            AdministratorShellResult::Unclear(None)
        );
    }

    fn report(
        success: bool,
        status: Option<i32>,
        stderr: &str,
        step_count: usize,
    ) -> AdministratorShellReport {
        administrator_shell_report(success, status, stderr, step_count).unwrap()
    }

    #[test]
    fn a_shell_status_names_the_step_that_stopped_the_run() {
        assert_eq!(
            report(true, Some(0), "", 2),
            AdministratorShellReport {
                outcome: ShellLinesOutcome::Completed,
                error_text: None,
            }
        );
        assert_eq!(
            report(
                false,
                Some(1),
                "0:310: execution error: rm: /Applications/Surge XT.app: Operation not permitted (102)\n",
                2
            ),
            AdministratorShellReport {
                outcome: ShellLinesOutcome::FailedAt { index: 1 },
                error_text: Some("rm: /Applications/Surge XT.app: Operation not permitted".into()),
            }
        );
        // A step that stops the run without a word: osascript's own
        // sentence for that is not the step's message.
        assert_eq!(
            report(
                false,
                Some(1),
                "0:57: execution error: The command exited with a non-zero status. (102)\n",
                2
            ),
            AdministratorShellReport {
                outcome: ShellLinesOutcome::FailedAt { index: 1 },
                error_text: None,
            }
        );
        assert_eq!(
            report(false, Some(1), "0:9: execution error: (101)", 1).outcome,
            ShellLinesOutcome::FailedAt { index: 0 }
        );
        assert_eq!(
            report(false, Some(1), "0:9: execution error: x (254)", 154).outcome,
            ShellLinesOutcome::FailedAt { index: 153 }
        );
        // 100 is the `cd`, 255 is `do shell script`'s own, and 103 names no
        // step of a two-step run: none of these says which steps ran.
        for (status, step_count) in [(100, 2), (255, 154), (103, 2), (1, 2), (2, 2)] {
            assert_eq!(
                report(
                    false,
                    Some(1),
                    &format!("0:9: execution error: x ({status})"),
                    step_count
                )
                .outcome,
                ShellLinesOutcome::Stopped { code: Some(status) },
                "{status}"
            );
        }
        // No number at all: not even a status to go by, but the text is
        // still worth keeping.
        assert_eq!(
            report(false, Some(1), "something went wrong\n", 2),
            AdministratorShellReport {
                outcome: ShellLinesOutcome::Stopped { code: None },
                error_text: Some("something went wrong".into()),
            }
        );
        assert_eq!(
            report(false, None, "", 2),
            AdministratorShellReport {
                outcome: ShellLinesOutcome::Stopped { code: None },
                error_text: None,
            }
        );
    }

    #[test]
    fn a_cancelled_or_failed_prompt_is_an_error_because_nothing_ran() {
        assert!(matches!(
            administrator_shell_report(
                false,
                Some(1),
                "0:28: execution error: User canceled. (-128)\n",
                3
            ),
            Err(ElevationError::UserCancelledElevation { .. })
        ));
        assert!(matches!(
            administrator_shell_report(
                false,
                Some(1),
                "0:120: execution error: The authorization was denied. (-60005)\n",
                3
            ),
            Err(ElevationError::PromptFailed { number: -60005 })
        ));
    }

    #[test]
    fn error_text_keeps_the_message_without_osascripts_additions() {
        assert_eq!(
            shell_error_text("0:57: execution error: rm: a: Operation not permitted (101)\n")
                .as_deref(),
            Some("rm: a: Operation not permitted")
        );
        // `do shell script` hands back the shell's line feeds as returns.
        assert_eq!(
            shell_error_text("0:57: execution error: rm: a: one\rrm: b: two (101)\n").as_deref(),
            Some("rm: a: one\nrm: b: two")
        );
        // A message in another label keeps its words, without the position.
        assert_eq!(
            shell_error_text("12:34: Ausführungsfehler: rm: a (101)").as_deref(),
            Some("Ausführungsfehler: rm: a")
        );
        // A program's own `name: text` is never taken for the position.
        assert_eq!(
            shell_error_text("rm: a: Operation not permitted").as_deref(),
            Some("rm: a: Operation not permitted")
        );
        // Log lines osascript itself prints before its error are dropped.
        assert_eq!(
            shell_error_text(
                "2026-11-07 15:30:12.345 osascript[123:4567] a log line\n0:57: execution error: rm: a: Operation not permitted (101)\n"
            )
            .as_deref(),
            Some("rm: a: Operation not permitted")
        );
        // Only the number at the very end is osascript's.
        assert_eq!(
            shell_error_text("0:9: execution error: failed (3) (101)").as_deref(),
            Some("failed (3)")
        );
        // A later line of the message that looks like a position stays in it.
        assert_eq!(
            shell_error_text("0:57: execution error: first\r10:30: second (101)").as_deref(),
            Some("first\n10:30: second")
        );
        // The last entry is what osascript says itself when the step wrote
        // nothing: AppleScript's words, not the step's.
        for nothing in [
            "",
            "\n",
            "0:9: execution error:  (101)",
            "(101)",
            "0:57: execution error: The command exited with a non-zero status. (102)\n",
        ] {
            assert_eq!(shell_error_text(nothing), None, "{nothing:?}");
        }
    }

    #[test]
    fn error_text_is_cut_at_8_kb_on_a_character_boundary() {
        // Two-byte characters with one byte in front, so 8 KB falls in the
        // middle of one.
        let long = format!("0:9: execution error: x{} (101)", "é".repeat(10_000));
        let text = shell_error_text(&long).unwrap();
        assert!(text.len() <= SHELL_ERROR_TEXT_LIMIT);
        assert!(text.len() >= SHELL_ERROR_TEXT_LIMIT - 1);
        assert!(text.starts_with("xé"));
    }

    #[test]
    fn a_slow_answer_gets_one_still_waiting_call() {
        let mut calls = 0;
        let answer = wait_with_notice(
            || {
                std::thread::sleep(Duration::from_millis(300));
                7
            },
            Duration::from_millis(1),
            &mut || calls += 1,
        );
        assert_eq!(answer, Some(7));
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_quick_answer_gets_no_still_waiting_call() {
        let mut calls = 0;
        let answer = wait_with_notice(|| 7, Duration::from_secs(600), &mut || calls += 1);
        assert_eq!(answer, Some(7));
        assert_eq!(calls, 0);
    }

    /// The request holds a relative program, which the shell rules refuse
    /// too. So if the guard ever went missing, this test would still never
    /// reach `osascript`: it would fail with the other reason instead.
    #[test]
    fn the_administrator_shell_never_prompts_from_a_test_build() {
        let request = AdministratorShellRequest {
            steps: vec![run("rm", &["-R", "/tmp/never"])],
        };
        let mut waits = 0;
        let mut still_waiting = || waits += 1;
        match run_administrator_shell(&request, Duration::ZERO, &mut still_waiting) {
            Err(ElevationError::RefusedCommand { program, reason }) => {
                assert_eq!(reason, TEST_BUILD_REFUSAL);
                assert_eq!(program, Path::new("/usr/bin/osascript"));
            }
            other => panic!("expected the test-build refusal, got {other:?}"),
        }
        // Even an empty request, which would never prompt, is refused.
        assert!(matches!(
            run_administrator_shell(
                &AdministratorShellRequest::default(),
                Duration::ZERO,
                &mut still_waiting
            ),
            Err(ElevationError::RefusedCommand {
                reason: TEST_BUILD_REFUSAL,
                ..
            })
        ));
        assert_eq!(waits, 0);
    }

    /// The same for the macOS install path. Only macOS has the guard there:
    /// on Windows this would raise a UAC prompt, so it never runs there.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_macos_elevated_install_never_prompts_from_a_test_build() {
        match super::run_elevated_and_wait(Path::new("true"), &[], None, ElevatedWindow::Hidden) {
            Err(ElevationError::RefusedCommand { reason, .. }) => {
                assert_eq!(reason, TEST_BUILD_REFUSAL)
            }
            other => panic!("expected the test-build refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_error_texts_name_no_platform() {
        let program = Path::new("/usr/sbin/installer").to_path_buf();
        for error in [
            ElevationError::LaunchFailed {
                program: program.clone(),
                code: 2,
            },
            ElevationError::UserCancelledElevation {
                program: program.clone(),
            },
            ElevationError::PromptFailed { number: -60005 },
            ElevationError::RefusedCommand {
                program,
                reason: NOT_RABBIT_REFUSAL,
            },
        ] {
            let text = error.to_string();
            for platform_word in ["Windows", "Win32", "ShellExecute", "UAC", "macOS"] {
                assert!(!text.contains(platform_word), "{text}");
            }
        }
    }

    #[test]
    fn rabbit_builds_are_recognised_by_every_name_they_ship_under() {
        for stem in [
            "rabbit",
            "RABBIT",
            "rabbit-windows-x86_64",
            "rabbit-windows-aarch64",
            "rabbit-0.5.0-windows-x86_64",
            "Rabbit-0.4.4-windows-aarch64",
            "rabbit-windows-x86_64 (1)",
            "rabbit (2)",
            "rabbit-macos-universal",
            "rabbit-macos-aarch64",
        ] {
            assert!(super::is_rabbit_file_stem(stem), "{stem}");
        }
    }

    /// macOS runs the app bundle's `Contents/MacOS/rabbit`, and the bare
    /// binary under its download name, which has dots in it: the stem of
    /// `rabbit-0.5.0-macos-universal` is `rabbit-0.5`.
    #[test]
    fn macos_builds_are_recognised_by_the_paths_they_run_from() {
        for program in [
            "/Applications/Rabbit.app/Contents/MacOS/rabbit",
            "/Users/toni/Downloads/Rabbit/Rabbit.app/Contents/MacOS/rabbit",
            "/private/var/folders/xy/abc/T/AppTranslocation/0A1B2C3D/d/Rabbit.app/Contents/MacOS/rabbit",
            "/Users/toni/Downloads/rabbit-0.5.0-macos-universal",
            "/Users/toni/Downloads/rabbit-0.5.0-1a2b3c4-macos-aarch64",
            "/Users/toni/Downloads/rabbit-0.5.0-macos-x86_64 (1)",
            "/Users/toni/Downloads/rabbit-macos-universal",
            "/usr/local/bin/rabbit",
        ] {
            assert!(super::is_rabbit_program(Path::new(program)), "{program}");
        }
        // Only Windows splits paths at `\`; elsewhere the whole string is one
        // file name and its stem starts with `C:\`.
        #[cfg(windows)]
        assert!(super::is_rabbit_program(Path::new(
            r"C:\Users\toni\Downloads\rabbit-0.5.0-windows-x86_64.exe"
        )));
        for program in [
            "/Users/runner/work/rabbit/target/debug/deps/rabbit_platform-0123456789abcdef",
            "/Users/runner/work/rabbit/target/debug/deps/rabbit-0123456789abcdef",
            "/Users/runner/work/rabbit/target/debug/deps/rabbit_core-fedcba9876543210",
            "/var/folders/xy/T/rustdoctestXYZ/rust_out",
        ] {
            assert!(!super::is_rabbit_program(Path::new(program)), "{program}");
        }
    }

    #[test]
    fn test_binaries_and_other_programs_are_not_rabbit() {
        for stem in [
            // cargo's test binaries: the `rabbit` crate's own, the
            // libraries', and an integration test's.
            "rabbit-0123456789abcdef",
            "rabbit-0123456789ABCDEF",
            "rabbit_core-0123456789abcdef",
            "rabbit_platform-fedcba9876543210",
            "rabbit_ui_wxdragon-0011223344556677",
            "uninstall_cli-0123456789abcdef",
            // rustdoc's doctest binaries.
            "rust_out",
            "rabbitry",
            "my-rabbit",
            "",
        ] {
            assert!(!super::is_rabbit_file_stem(stem), "{stem}");
        }
    }

    /// This test binary is `rabbit_platform-<hash>.exe`, so the guard in
    /// `spawn_elevated` holds here. The test never calls `spawn_elevated`
    /// itself: if the guard were broken, that would raise a real prompt.
    #[test]
    fn a_test_binary_never_counts_as_rabbit() {
        assert!(!super::running_as_rabbit());
    }

    #[cfg(windows)]
    #[test]
    fn the_current_process_can_tell_whether_it_is_elevated() {
        // Either answer is right; the token has to be readable.
        super::is_current_process_elevated().unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn elevation_state_is_windows_only() {
        assert_eq!(
            super::is_current_process_elevated().unwrap_err().kind(),
            std::io::ErrorKind::Unsupported
        );
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

    /// The generated text, run for real by an unprivileged `/bin/sh` and by
    /// `osascript` without administrator privileges. Nothing here can raise
    /// a password window: the shell runs as the test's own user, and the
    /// AppleScript builder below has no way to ask for administrator
    /// privileges. Every child has a time limit, so a stuck one fails the
    /// test instead of hanging CI.
    #[cfg(target_os = "macos")]
    mod macos_shell {
        use super::super::{
            OSASCRIPT, ShellLinesOutcome, administrator_shell_report, administrator_shell_text,
            applescript_quote,
        };
        use super::{expecting_no_match, run, tolerated};
        use std::path::{Path, PathBuf};
        use std::process::{Child, Command, Output, Stdio};
        use std::time::Duration;

        const LIMIT: Duration = Duration::from_secs(120);

        /// Run `command` and collect its output, killing it and failing the
        /// test if it takes longer than `LIMIT`.
        fn output_within_limit(command: &mut Command) -> Output {
            let child = command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("the program starts");
            let pid = child.id();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = sender.send(child.wait_with_output());
            });
            match receiver.recv_timeout(LIMIT) {
                Ok(output) => output.expect("the program's output can be read"),
                Err(_) => {
                    // SAFETY: kill only sends a signal, to our own child,
                    // which nobody has waited for yet.
                    unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
                    panic!("{command:?} didn't finish within {LIMIT:?}");
                }
            }
        }

        fn run_unprivileged_shell(shell: &str) -> Output {
            output_within_limit(Command::new("/bin/sh").arg("-c").arg(shell))
        }

        /// The AppleScript for `shell` without administrator privileges.
        /// Test-only, and unable to ask for them: it never adds the
        /// parameter, and refuses text that would.
        fn unprivileged_script(shell: &str) -> String {
            let script = format!("do shell script {}", applescript_quote(shell));
            assert!(!script.contains("administrator privileges"));
            script
        }

        fn run_unprivileged_osascript(shell: &str) -> Output {
            output_within_limit(
                Command::new(OSASCRIPT)
                    .arg("-e")
                    .arg(unprivileged_script(shell)),
            )
        }

        /// A temp folder by its real path: `/var` is a link to
        /// `/private/var`, and root's removals are given real paths.
        fn real_temp_dir() -> (tempfile::TempDir, PathBuf) {
            let dir = tempfile::tempdir().unwrap();
            let real = dir.path().canonicalize().unwrap();
            (dir, real)
        }

        fn text(path: &Path) -> &str {
            path.to_str().unwrap()
        }

        /// Ends the child when the test ends, however it ends.
        struct EndOnDrop(Child);

        impl Drop for EndOnDrop {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        #[test]
        fn rm_removes_a_bundle_with_links_inside_and_leaves_their_targets_alone() {
            let (_dir, root) = real_temp_dir();
            let outside = root.join("outside");
            std::fs::create_dir(&outside).unwrap();
            std::fs::write(outside.join("keep.txt"), "kept").unwrap();
            let bundle = root.join("Surge XT.app");
            let macos = bundle.join("Contents").join("MacOS");
            std::fs::create_dir_all(&macos).unwrap();
            std::fs::write(macos.join("Surge XT"), "binary").unwrap();
            std::os::unix::fs::symlink(&outside, bundle.join("Contents").join("linked folder"))
                .unwrap();
            std::os::unix::fs::symlink(
                outside.join("keep.txt"),
                bundle.join("Contents").join("linked file"),
            )
            .unwrap();

            let shell =
                administrator_shell_text(&[run("/bin/rm", &["-R", "-f", "-x", text(&bundle)])])
                    .unwrap();
            let output = run_unprivileged_shell(&shell);

            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(std::fs::symlink_metadata(&bundle).is_err());
            assert_eq!(
                std::fs::read_to_string(outside.join("keep.txt")).unwrap(),
                "kept"
            );
        }

        #[test]
        fn a_failing_step_stops_the_run_with_its_own_status() {
            let (_dir, root) = real_temp_dir();
            let marker = root.join("ran");
            let shell = administrator_shell_text(&[
                run("/usr/bin/true", &[]),
                run("/usr/bin/false", &[]),
                run("/usr/bin/touch", &[text(&marker)]),
            ])
            .unwrap();
            let output = run_unprivileged_shell(&shell);
            assert_eq!(output.status.code(), Some(102), "{output:?}");
            assert!(!marker.exists());
        }

        #[test]
        fn a_tolerated_failure_is_silent_and_the_run_goes_on() {
            let (_dir, root) = real_temp_dir();
            let marker = root.join("ran");
            let shell = administrator_shell_text(&[
                tolerated("/bin/ls", &[text(&root.join("missing"))]),
                run("/usr/bin/touch", &[text(&marker)]),
            ])
            .unwrap();
            let output = run_unprivileged_shell(&shell);
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            assert!(marker.exists());
        }

        #[test]
        fn nothing_a_step_prints_reaches_standard_output() {
            let shell = administrator_shell_text(&[run("/bin/echo", &["noise"])]).unwrap();
            let output = run_unprivileged_shell(&shell);
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
        }

        #[test]
        fn the_no_match_step_passes_only_when_nothing_matches() {
            let (_dir, root) = real_temp_dir();
            let marker = root.join("ran");
            let then_touch = run("/usr/bin/touch", &[text(&marker)]);

            // No process can have this name: pgrep exits 1, the run goes on.
            let shell = administrator_shell_text(&[
                expecting_no_match("/usr/bin/pgrep", &["-x", "-i", "rabbitnosuchprog"]),
                then_touch.clone(),
            ])
            .unwrap();
            let output = run_unprivileged_shell(&shell);
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(marker.exists());
            std::fs::remove_file(&marker).unwrap();

            // A process named `sleep` runs, matched in other case, which
            // also proves -x and -i: pgrep exits 0 and the run stops here.
            let _sleeper = EndOnDrop(
                Command::new("/bin/sleep")
                    .arg("120")
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let shell = administrator_shell_text(&[
                expecting_no_match("/usr/bin/pgrep", &["-x", "-i", "SLEEP"]),
                then_touch.clone(),
            ])
            .unwrap();
            let output = run_unprivileged_shell(&shell);
            assert_eq!(output.status.code(), Some(101), "{output:?}");
            assert!(!marker.exists());

            // pgrep's own error (a pattern it can't read, status 2) stops
            // the run too: the step fails closed.
            let shell = administrator_shell_text(&[
                expecting_no_match("/usr/bin/pgrep", &["-x", "("]),
                then_touch,
            ])
            .unwrap();
            let output = run_unprivileged_shell(&shell);
            assert_eq!(output.status.code(), Some(101), "{output:?}");
            assert!(!marker.exists());
        }

        /// An uninstall's pgrep step puts `--` before its pattern, so no
        /// program name can ever be read as an option. pgrep takes that:
        /// the pattern after it is a pattern even when it starts with a
        /// dash, and it still matches as usual.
        #[test]
        fn pgrep_takes_its_pattern_after_a_double_dash() {
            let pgrep = |pattern: &str| {
                output_within_limit(
                    Command::new("/usr/bin/pgrep").args(["-x", "-i", "--", pattern]),
                )
            };
            // Nothing matches: 1, not a usage error (2).
            let output = pgrep("-rabbitnosuchprog|rabbitnosuchprog");
            assert_eq!(output.status.code(), Some(1), "{output:?}");

            let _sleeper = EndOnDrop(
                Command::new("/bin/sleep")
                    .arg("120")
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let output = pgrep("rabbitnosuchprog|SLEEP");
            assert_eq!(output.status.code(), Some(0), "{output:?}");
        }

        /// The steps that move REAPER's app into the user's Trash, run as
        /// the user on a temp folder: `test -d` and `test ! -L` stop at a
        /// Trash that is a link to a folder, and `mv -h -n` never replaces
        /// a file or a link that already has the name it was given, nor
        /// follows the link. Whatever status mv gives then, nothing moves,
        /// which is what the run checks afterwards.
        #[test]
        fn the_trash_move_steps_stop_at_a_linked_trash_and_never_replace_anything() {
            let (_dir, root) = real_temp_dir();
            let app = root.join("REAPER.app");
            std::fs::create_dir_all(app.join("Contents")).unwrap();
            let trash = root.join("Trash");
            std::fs::create_dir(&trash).unwrap();
            let elsewhere = root.join("elsewhere");
            std::fs::create_dir(&elsewhere).unwrap();
            let linked_trash = root.join("Linked Trash");
            std::os::unix::fs::symlink(&elsewhere, &linked_trash).unwrap();

            let steps = |trash: &Path, destination: &Path| {
                administrator_shell_text(&[
                    run("/bin/test", &["-d", text(trash)]),
                    run("/bin/test", &["!", "-L", text(trash)]),
                    run("/bin/mv", &["-h", "-n", text(&app), text(destination)]),
                ])
                .unwrap()
            };

            // A link to a folder passes `test -d`, and `test ! -L` stops it.
            let output =
                run_unprivileged_shell(&steps(&linked_trash, &linked_trash.join("REAPER x.app")));
            assert_eq!(output.status.code(), Some(102), "{output:?}");
            assert!(app.is_dir());
            assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);

            // The name is taken, by a file or by a link to a folder: nothing
            // moves, nothing is replaced, and the link isn't followed.
            let taken_by_file = trash.join("REAPER file.app");
            std::fs::write(&taken_by_file, "mine").unwrap();
            let output = run_unprivileged_shell(&steps(&trash, &taken_by_file));
            assert!(app.is_dir(), "{output:?}");
            assert_eq!(std::fs::read_to_string(&taken_by_file).unwrap(), "mine");
            let taken_by_link = trash.join("REAPER link.app");
            std::os::unix::fs::symlink(&elsewhere, &taken_by_link).unwrap();
            let output = run_unprivileged_shell(&steps(&trash, &taken_by_link));
            assert!(app.is_dir(), "{output:?}");
            assert!(
                std::fs::symlink_metadata(&taken_by_link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);

            // A fresh name: the app moves, and only it.
            let fresh = trash.join("REAPER 2026-11-07 15.30.12.app");
            let output = run_unprivileged_shell(&steps(&trash, &fresh));
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(!app.exists());
            assert!(fresh.join("Contents").is_dir());
        }

        fn osascript_report(shell: &str, step_count: usize) -> super::AdministratorShellReport {
            let output = run_unprivileged_osascript(shell);
            administrator_shell_report(
                output.status.success(),
                output.status.code(),
                &String::from_utf8_lossy(&output.stderr),
                step_count,
            )
            .unwrap_or_else(|error| panic!("{error} from {output:?}"))
        }

        #[test]
        fn osascript_reports_a_completed_shell() {
            let shell = administrator_shell_text(&[run("/usr/bin/true", &[])]).unwrap();
            let report = osascript_report(&shell, 1);
            assert_eq!(report.outcome, ShellLinesOutcome::Completed);
            assert_eq!(report.error_text, None);
        }

        #[test]
        fn osascript_reports_the_failing_step_and_its_message() {
            let steps = [
                run("/usr/bin/true", &[]),
                run(
                    "/bin/sh",
                    &["-c", "echo 'rm: /x: Operation not permitted' >&2; exit 1"],
                ),
            ];
            let shell = administrator_shell_text(&steps).unwrap();
            let report = osascript_report(&shell, steps.len());
            assert_eq!(report.outcome, ShellLinesOutcome::FailedAt { index: 1 });
            assert_eq!(
                report.error_text.as_deref(),
                Some("rm: /x: Operation not permitted")
            );
        }

        #[test]
        fn osascript_keeps_every_line_of_the_message() {
            let steps = [run(
                "/bin/sh",
                &["-c", r"printf 'first line\nsecond line\n' >&2; exit 1"],
            )];
            let shell = administrator_shell_text(&steps).unwrap();
            let report = osascript_report(&shell, steps.len());
            assert_eq!(report.outcome, ShellLinesOutcome::FailedAt { index: 0 });
            let text = report.error_text.unwrap();
            assert!(text.contains("first line"), "{text:?}");
            assert!(text.contains("second line"), "{text:?}");
            assert!(!text.contains('\r'), "{text:?}");
        }

        #[test]
        fn osascript_never_mixes_standard_output_into_the_message() {
            let steps = [
                run("/bin/echo", &["noise from a step"]),
                run("/usr/bin/false", &[]),
            ];
            let shell = administrator_shell_text(&steps).unwrap();
            let report = osascript_report(&shell, steps.len());
            assert_eq!(report.outcome, ShellLinesOutcome::FailedAt { index: 1 });
            assert!(
                !report
                    .error_text
                    .as_deref()
                    .unwrap_or_default()
                    .contains("noise"),
                "{report:?}"
            );
            // `false` writes nothing to its error output, so all osascript
            // has is its own sentence, which is not the step's message.
            assert_eq!(report.error_text, None, "{report:?}");
        }

        #[test]
        fn osascript_status_255_is_read_as_stopped() {
            let report = osascript_report("exit 255", 154);
            assert_eq!(
                report.outcome,
                ShellLinesOutcome::Stopped { code: Some(255) }
            );
        }
    }
}
