//! Shared spawning helpers for the places RABBIT shells out to a helper
//! program.
//!
//! RABBIT's release GUI build targets the Windows GUI subsystem, so the
//! process owns no console. That is what keeps a console from flashing up
//! when the user double-clicks RABBIT — but it also means that every time
//! RABBIT launches a *console* program, Windows has no console to lend the
//! child and creates a fresh one for it. The user sees a black window pop
//! up and vanish, usually mid-install, with nothing to explain it.
//!
//! Every helper RABBIT runs on Windows is a console program whose output it
//! reads through pipes, never something the user is meant to watch, so none
//! of them should get a window. The vendor installers are the exception,
//! and they are GUI programs launched through
//! [`crate::elevation::run_elevated_and_wait`] instead.

use std::process::Command;

/// Run a console-subsystem child with no console window.
///
/// Only the console *window* is suppressed. Handles redirected through
/// `Stdio` pipes still work, which is how every caller reads its results —
/// so this changes what the user sees and nothing else.
///
/// No-op off Windows, where a child process never brings its own terminal.
pub fn without_console_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        /// `CREATE_NO_WINDOW` from `winbase.h`. Hardcoded rather than
        /// pulled from `windows-sys` because this crate's Windows surface
        /// is otherwise the Shell and VersionInfo APIs, and one process
        /// creation flag does not justify another feature on that
        /// dependency.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The flag must suppress the console window without costing the caller
    /// the output it was after. Every site that uses this reads the child
    /// through a pipe, so a version of this that also detached stdout would
    /// silently turn signature checks and Defender queries into no-ops.
    #[test]
    fn a_windowless_child_still_delivers_its_piped_output() {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "echo", "rabbit"]);
            command
        } else {
            let mut command = Command::new("/bin/echo");
            command.arg("rabbit");
            command
        };

        let output = without_console_window(&mut command)
            .output()
            .expect("the helper process should run");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "rabbit");
    }
}
