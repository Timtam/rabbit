//! Running a program with a time limit, for RABBIT's elevated uninstall
//! helper.
//!
//! The helper already has administrator rights, so it starts a vendor
//! uninstaller as an ordinary child process, with no second permission
//! prompt. An uninstaller can stop on a dialog nobody answers, so RABBIT
//! never waits for one forever: after a while it says it is still waiting,
//! and at the time limit it stops waiting and says so, leaving the program
//! running.
//!
//! NSIS uninstallers take one argument that ordinary quoting would break;
//! see [`NsisInPlaceArgument`].

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// How often a running child is checked. Short enough that an exit is
/// noticed at once, long enough to cost nothing.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// NSIS's `_?=<folder>` argument, checked so it reaches the uninstaller
/// exactly as NSIS reads it.
///
/// `_?=` tells an NSIS uninstaller which folder to remove and stops it from
/// copying itself to the temporary folder and exiting at once. NSIS reads it
/// straight from the command line to its end, so it must come last and must
/// not be quoted, even when the folder has spaces (NSIS documentation, 3.2.2
/// and 4.12). [`TimedCommand::nsis_in_place`] always puts it last and passes
/// it unquoted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NsisInPlaceArgument {
    text: String,
}

impl NsisInPlaceArgument {
    /// The argument for `install_dir`. Refused, as `InvalidInput`, when the
    /// folder isn't an absolute path, isn't valid Unicode, contains a
    /// double quote, a NUL, a line break or any other control character, or
    /// starts or ends with a space: none of these can be passed unquoted
    /// and still arrive as the same folder.
    pub fn new(install_dir: &Path) -> io::Result<Self> {
        let refuse = |why: &str| {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} can't be an NSIS install folder: {why}",
                    install_dir.display()
                ),
            ))
        };
        let Some(folder) = install_dir.to_str() else {
            return refuse("it isn't valid Unicode");
        };
        if !install_dir.is_absolute() {
            return refuse("it isn't an absolute path");
        }
        if folder.contains(|ch: char| ch == '"' || ch.is_control()) {
            return refuse("it contains a double quote, a line break or another control character");
        }
        if folder.starts_with(char::is_whitespace) || folder.ends_with(char::is_whitespace) {
            return refuse("it starts or ends with a space");
        }
        Ok(Self {
            text: format!("_?={folder}"),
        })
    }

    /// The whole argument, `_?=` included.
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// What a [`TimedCommand::run`] came to.
#[derive(Debug)]
pub enum TimedRunOutcome {
    /// The program ended with this exit code. On Unix, a program ended by a
    /// signal reads as 128 plus the signal, as shells report it.
    Exited(i32),
    /// The program was still running at the time limit. It keeps running;
    /// the caller decides whether to wait longer or let it be.
    TimedOut(Child),
}

/// A program to run with a time limit. Not elevated: the elevated helper
/// uses this for programs it starts with the rights it already has.
///
/// The child gets no standard input or output, and RABBIT's own secrets are
/// removed from its environment (see
/// [`crate::process::without_rabbit_secrets`]). Its window, if it has one,
/// shows as usual.
#[derive(Debug, Clone)]
pub struct TimedCommand {
    program: PathBuf,
    arguments: Vec<OsString>,
    nsis_in_place: Option<NsisInPlaceArgument>,
    working_directory: Option<PathBuf>,
    environment: Vec<(OsString, OsString)>,
}

impl TimedCommand {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            nsis_in_place: None,
            working_directory: None,
            environment: Vec::new(),
        }
    }

    /// One argument, quoted as Windows programs expect when it needs it.
    pub fn arg(&mut self, argument: impl Into<OsString>) -> &mut Self {
        self.arguments.push(argument.into());
        self
    }

    pub fn args<I, S>(&mut self, arguments: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.arguments.extend(arguments.into_iter().map(Into::into));
        self
    }

    /// End the command line with NSIS's `_?=` argument, unquoted, after
    /// every other argument whenever they were added. Windows only: a run
    /// with it fails as `Unsupported` elsewhere.
    pub fn nsis_in_place(&mut self, argument: NsisInPlaceArgument) -> &mut Self {
        self.nsis_in_place = Some(argument);
        self
    }

    pub fn current_dir(&mut self, directory: impl Into<PathBuf>) -> &mut Self {
        self.working_directory = Some(directory.into());
        self
    }

    pub fn env(&mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> &mut Self {
        self.environment.push((key.into(), value.into()));
        self
    }

    /// Start the program and wait for it, up to `timeout`.
    ///
    /// `on_still_waiting` is called once, when the program has run for
    /// `still_waiting_after` and hasn't ended, so the caller can say it is
    /// still waiting and that the program may be showing a window. An error
    /// means the program couldn't be started or checked on.
    pub fn run(
        &self,
        timeout: Duration,
        still_waiting_after: Duration,
        on_still_waiting: &mut dyn FnMut(),
    ) -> io::Result<TimedRunOutcome> {
        let mut command = self.command()?;
        let mut child = command.spawn()?;
        let started = Instant::now();
        let mut said_still_waiting = false;
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(TimedRunOutcome::Exited(exit_code(status)));
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Ok(TimedRunOutcome::TimedOut(child));
            }
            if !said_still_waiting && elapsed >= still_waiting_after {
                said_still_waiting = true;
                on_still_waiting();
            }
            std::thread::sleep(POLL_INTERVAL.min(timeout - elapsed));
        }
    }

    fn command(&self) -> io::Result<Command> {
        let mut command = Command::new(&self.program);
        command
            .args(&self.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(directory) = &self.working_directory {
            command.current_dir(directory);
        }
        for (key, value) in &self.environment {
            command.env(key, value);
        }
        crate::process::without_rabbit_secrets(&mut command);
        if let Some(argument) = &self.nsis_in_place {
            append_unquoted(&mut command, argument)?;
        }
        Ok(command)
    }
}

#[cfg(windows)]
fn append_unquoted(command: &mut Command, argument: &NsisInPlaceArgument) -> io::Result<()> {
    use std::os::windows::process::CommandExt;

    // raw_arg appends the text after one space, exactly as given. It is
    // added last, so nothing can follow it.
    command.raw_arg(argument.as_str());
    Ok(())
}

#[cfg(not(windows))]
fn append_unquoted(_command: &mut Command, _argument: &NsisInPlaceArgument) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "NSIS uninstallers only run on Windows",
    ))
}

#[cfg(windows)]
fn exit_code(status: ExitStatus) -> i32 {
    // Windows always has a code; the fallback is never reached.
    status.code().unwrap_or(-1)
}

#[cfg(unix)]
fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;

    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(-1)
}

#[cfg(not(any(windows, unix)))]
fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute(windows: &str, unix: &str) -> PathBuf {
        PathBuf::from(if cfg!(windows) { windows } else { unix })
    }

    #[test]
    fn the_nsis_argument_keeps_a_folder_with_spaces_as_it_is() {
        let folder = absolute("C:\\Program Files\\REAPER (x64)", "/Program Files/REAPER");
        let argument = NsisInPlaceArgument::new(&folder).unwrap();
        assert_eq!(
            argument.as_str(),
            format!("_?={}", folder.to_str().unwrap())
        );
    }

    #[test]
    fn the_nsis_argument_refuses_what_it_could_not_pass_unquoted() {
        for folder in [
            absolute("C:\\Program Files\\RE\"APER", "/Program Files/RE\"APER"),
            absolute("C:\\Program Files\\RE\0APER", "/Program Files/RE\0APER"),
            absolute("C:\\Program Files\\RE\nAPER", "/Program Files/RE\nAPER"),
            absolute("C:\\Program Files\\RE\rAPER", "/Program Files/RE\rAPER"),
            absolute("C:\\Program Files\\RE\tAPER", "/Program Files/RE\tAPER"),
            absolute("C:\\Program Files\\REAPER ", "/Program Files/REAPER "),
            PathBuf::from("Program Files\\REAPER"),
            PathBuf::new(),
        ] {
            let error = NsisInPlaceArgument::new(&folder).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{folder:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_unix_child_reports_its_exit_code_and_a_slow_one_times_out() {
        let mut quick = TimedCommand::new("/bin/sh");
        quick.args(["-c", "exit 3"]);
        let mut said = 0;
        let outcome = quick
            .run(
                Duration::from_secs(30),
                Duration::from_secs(30),
                &mut || said += 1,
            )
            .unwrap();
        assert!(matches!(outcome, TimedRunOutcome::Exited(3)), "{outcome:?}");
        assert_eq!(said, 0);

        let mut slow = TimedCommand::new("/bin/sleep");
        slow.arg("30");
        let outcome = slow
            .run(
                Duration::from_millis(600),
                Duration::from_millis(100),
                &mut || said += 1,
            )
            .unwrap();
        let TimedRunOutcome::TimedOut(mut child) = outcome else {
            panic!("expected a timeout, got {outcome:?}");
        };
        assert_eq!(said, 1);
        assert!(child.try_wait().unwrap().is_none());
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn the_nsis_argument_only_runs_on_windows() {
        let mut command = TimedCommand::new("/bin/sh");
        command.nsis_in_place(NsisInPlaceArgument::new(Path::new("/Program Files")).unwrap());
        let error = command
            .run(Duration::from_secs(1), Duration::from_secs(1), &mut || {})
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}

/// These start this test binary again as the child, so no third-party
/// program runs. The two "child" tests do nothing in a normal run; only the
/// environment variable the parent sets makes them act.
#[cfg(all(test, windows))]
mod windows_tests {
    use std::fs;

    use super::*;

    const REPORT_TO: &str = "RABBIT_TEST_REPORT_COMMAND_LINE_TO";
    const SLEEP_FOR_MS: &str = "RABBIT_TEST_SLEEP_MILLISECONDS";
    const REPORTING_CHILD: &str =
        "timed_process::windows_tests::child_reports_its_raw_command_line";
    const SLEEPING_CHILD: &str = "timed_process::windows_tests::child_sleeps";
    const CHILD_EXIT_CODE: i32 = 7;

    #[test]
    fn child_reports_its_raw_command_line() {
        let Some(report) = std::env::var_os(REPORT_TO) else {
            return;
        };
        // SAFETY: GetCommandLineW returns a NUL-terminated string that
        // lives as long as the process.
        let line = unsafe {
            let start = windows_sys::Win32::System::Environment::GetCommandLineW();
            let mut length = 0;
            while *start.add(length) != 0 {
                length += 1;
            }
            String::from_utf16(std::slice::from_raw_parts(start, length)).unwrap()
        };
        fs::write(report, line).unwrap();
        std::process::exit(CHILD_EXIT_CODE);
    }

    #[test]
    fn child_sleeps() {
        let Ok(milliseconds) = std::env::var(SLEEP_FOR_MS) else {
            return;
        };
        std::thread::sleep(Duration::from_millis(milliseconds.parse().unwrap()));
    }

    /// This test binary, told to run only the child test `name`. Anything
    /// the test adds after these arguments reads to the test harness as
    /// more test names, which match nothing.
    fn this_binary_running(name: &str) -> TimedCommand {
        let mut command = TimedCommand::new(std::env::current_exe().unwrap());
        command.args(["--exact", name, "--test-threads=1", "--quiet"]);
        command
    }

    #[test]
    fn the_nsis_folder_comes_last_unquoted_after_normally_quoted_arguments() {
        let scratch = tempfile::tempdir().unwrap();
        let report = scratch.path().join("command line.txt");
        let mut command = this_binary_running(REPORTING_CHILD);
        command
            .env(REPORT_TO, &report)
            .arg("/S")
            .nsis_in_place(
                NsisInPlaceArgument::new(Path::new("C:\\Some Folder\\With Spaces")).unwrap(),
            )
            // Added after the NSIS argument, and still placed before it.
            .arg("an argument with spaces");
        let mut said = 0;

        let outcome = command
            .run(
                Duration::from_secs(120),
                Duration::from_secs(120),
                &mut || said += 1,
            )
            .unwrap();

        assert!(
            matches!(outcome, TimedRunOutcome::Exited(CHILD_EXIT_CODE)),
            "{outcome:?}"
        );
        assert_eq!(said, 0, "it ended long before the still-waiting time");
        let line = fs::read_to_string(&report).unwrap();
        assert!(
            line.ends_with(" /S \"an argument with spaces\" _?=C:\\Some Folder\\With Spaces"),
            "{line}"
        );
    }

    #[test]
    fn a_child_still_running_at_the_time_limit_is_left_running() {
        let mut command = this_binary_running(SLEEPING_CHILD);
        command.env(SLEEP_FOR_MS, "60000");
        let mut said = 0;
        let started = Instant::now();

        let outcome = command
            .run(
                Duration::from_millis(1500),
                Duration::from_millis(300),
                &mut || said += 1,
            )
            .unwrap();

        let TimedRunOutcome::TimedOut(mut child) = outcome else {
            panic!("expected a timeout, got {outcome:?}");
        };
        assert!(started.elapsed() >= Duration::from_millis(1500));
        assert_eq!(said, 1, "said once that it is still waiting");
        assert!(child.try_wait().unwrap().is_none(), "still running");
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn a_program_that_cannot_start_is_an_error() {
        let scratch = tempfile::tempdir().unwrap();
        let error = TimedCommand::new(scratch.path().join("missing.exe"))
            .run(Duration::from_secs(5), Duration::from_secs(5), &mut || {})
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
