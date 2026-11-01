//! Ctrl+C, caught for as long as a command needs it.
//!
//! An uninstall moves one package's files into a backup set, then saves the
//! receipt. Ended in between, it leaves the files in the set and a receipt
//! that still lists them. A restore can sort that out, but stopping between
//! two packages leaves nothing to sort out. While a [`CtrlCGuard`] lives,
//! the first Ctrl+C only sets a flag the command reads at its next safe
//! point, and prints a notice saying so. A second Ctrl+C ends the process as
//! usual, for a user who really wants out.
//!
//! Windows calls a console handler on a thread of its own. Unix runs a
//! signal handler on whichever thread the signal lands on, where only
//! async-signal-safe calls are allowed. So both handlers do no more than set
//! an atomic flag and hand the notice to one `write`.
//!
//! The handler goes again when the last guard is dropped, so every other
//! command keeps Ctrl+C's usual meaning.

use std::io;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

/// Set by the first Ctrl+C while a guard lives.
static PRESSED: AtomicBool = AtomicBool::new(false);

/// The notice the handler prints, as a pointer and a length a signal
/// handler can read without taking a lock. Set before the handler is
/// installed, and left alone while it is.
static NOTICE_PTR: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static NOTICE_LEN: AtomicUsize = AtomicUsize::new(0);

/// How many guards live, and what to put back when the last one goes.
static STATE: Mutex<State> = Mutex::new(State {
    guards: 0,
    #[cfg(unix)]
    previous: None,
});

struct State {
    guards: usize,
    /// SIGINT's handling before RABBIT's handler took over, or `None` when
    /// RABBIT installed nothing.
    #[cfg(unix)]
    previous: Option<libc::sigaction>,
}

/// While this lives, Ctrl+C sets [`CtrlCGuard::flag`] instead of ending the
/// process. Dropping the last guard gives Ctrl+C back its usual meaning.
#[derive(Debug)]
pub struct CtrlCGuard {
    _private: (),
}

impl CtrlCGuard {
    /// Set once Ctrl+C was pressed. Static, because a signal handler can't
    /// reach anything else.
    pub fn flag(&self) -> &'static AtomicBool {
        &PRESSED
    }
}

impl Drop for CtrlCGuard {
    fn drop(&mut self) {
        let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
        state.guards -= 1;
        if state.guards == 0 {
            platform::uninstall(&mut state);
        }
    }
}

/// Catch Ctrl+C until the returned guard is dropped: the first press sets
/// the guard's flag and writes `notice` to stderr; the second ends the
/// process. Guards may overlap; the first one installs the handler and
/// clears the flag, the last one removes the handler. An error means
/// Ctrl+C keeps its usual meaning.
pub fn catch_ctrl_c(notice: &'static str) -> io::Result<CtrlCGuard> {
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    if state.guards == 0 {
        PRESSED.store(false, Ordering::SeqCst);
        NOTICE_LEN.store(notice.len(), Ordering::SeqCst);
        NOTICE_PTR.store(notice.as_ptr().cast_mut(), Ordering::SeqCst);
        platform::install(&mut state)?;
    }
    state.guards += 1;
    Ok(CtrlCGuard { _private: () })
}

/// What both handlers do on a press. Returns `false` for a second press,
/// which should end the process.
fn on_press() -> bool {
    if PRESSED.swap(true, Ordering::SeqCst) {
        return false;
    }
    let pointer = NOTICE_PTR.load(Ordering::SeqCst);
    let length = NOTICE_LEN.load(Ordering::SeqCst);
    if !pointer.is_null() && length > 0 {
        platform::write_stderr(pointer, length);
    }
    true
}

#[cfg(windows)]
mod platform {
    use std::io;

    use windows_sys::Win32::Storage::FileSystem::WriteFile;
    use windows_sys::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_C_EVENT, GetStdHandle, STD_ERROR_HANDLE, SetConsoleCtrlHandler,
    };
    use windows_sys::core::BOOL;

    use super::State;

    /// Ctrl+C and Ctrl+Break are taken on the first press. Closing the
    /// console window, logging off and shutting down are left to Windows,
    /// which ends the process for them whatever a handler answers.
    pub(super) unsafe extern "system" fn on_console_event(event: u32) -> BOOL {
        if event != CTRL_C_EVENT && event != CTRL_BREAK_EVENT {
            return 0;
        }
        // FALSE passes the event on to the next handler, Windows' own, which
        // ends the process.
        BOOL::from(super::on_press())
    }

    pub(super) fn install(_state: &mut State) -> io::Result<()> {
        // SAFETY: the handler is a plain function that lives as long as the
        // process and only touches atomics.
        if unsafe { SetConsoleCtrlHandler(Some(on_console_event), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn uninstall(_state: &mut State) {
        // SAFETY: removes the handler `install` added; nothing else is
        // touched.
        unsafe { SetConsoleCtrlHandler(Some(on_console_event), 0) };
    }

    pub(super) fn write_stderr(pointer: *const u8, length: usize) {
        let length = u32::try_from(length).unwrap_or(u32::MAX);
        let mut written = 0_u32;
        // SAFETY: `pointer` and `length` describe a `&'static str`, and
        // `written` is a valid place for the count. A missing or redirected
        // stderr only makes the write fail, which is harmless.
        unsafe {
            WriteFile(
                GetStdHandle(STD_ERROR_HANDLE),
                pointer,
                length,
                &mut written,
                std::ptr::null_mut(),
            );
        }
    }
}

#[cfg(unix)]
mod platform {
    use std::io;

    use super::State;

    /// SIGINT, the signal Ctrl+C sends.
    pub(super) extern "C" fn on_sigint(_signal: libc::c_int) {
        // SA_RESETHAND has already put the default handling back, so the
        // next Ctrl+C ends the process whatever this returns.
        super::on_press();
    }

    pub(super) fn install(state: &mut State) -> io::Result<()> {
        // SAFETY: sigaction is plain data, for which all zeros is valid, and
        // each call gets valid pointers or null where the API allows it.
        unsafe {
            let mut current: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(libc::SIGINT, std::ptr::null(), &mut current) != 0 {
                return Err(io::Error::last_os_error());
            }
            // A shell that started RABBIT with Ctrl+C ignored, such as for a
            // background job, meant it: leave that alone.
            if current.sa_sigaction == libc::SIG_IGN {
                state.previous = None;
                return Ok(());
            }
            let mut action: libc::sigaction = std::mem::zeroed();
            let handler: extern "C" fn(libc::c_int) = on_sigint;
            action.sa_sigaction = handler as libc::sighandler_t;
            // SA_RESTART: a read or write the signal interrupts carries on,
            // rather than failing a file move with EINTR. SA_RESETHAND: the
            // second Ctrl+C gets the default handling and ends the process.
            action.sa_flags = libc::SA_RESTART | libc::SA_RESETHAND;
            libc::sigemptyset(&mut action.sa_mask);
            let mut previous: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(libc::SIGINT, &action, &mut previous) != 0 {
                return Err(io::Error::last_os_error());
            }
            state.previous = Some(previous);
        }
        Ok(())
    }

    pub(super) fn uninstall(state: &mut State) {
        if let Some(previous) = state.previous.take() {
            // SAFETY: puts back the action `install` read; a null pointer
            // for the old action is allowed.
            unsafe { libc::sigaction(libc::SIGINT, &previous, std::ptr::null_mut()) };
        }
    }

    pub(super) fn write_stderr(pointer: *const u8, length: usize) {
        // SAFETY: write is async-signal-safe, and `pointer` and `length`
        // describe a `&'static str`. A failed write is harmless.
        unsafe { libc::write(libc::STDERR_FILENO, pointer.cast(), length) };
    }
}

#[cfg(not(any(windows, unix)))]
mod platform {
    use std::io;

    use super::State;

    pub(super) fn install(_state: &mut State) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Ctrl+C can't be caught on this platform",
        ))
    }

    pub(super) fn uninstall(_state: &mut State) {}

    pub(super) fn write_stderr(_pointer: *const u8, _length: usize) {}
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::{catch_ctrl_c, on_press};

    /// The handler's own logic, called directly: actually pressing Ctrl+C
    /// would reach cargo and the shell too, and raising SIGINT twice ends
    /// the test process. One test, because the flag is the process's own.
    #[test]
    fn the_first_press_sets_the_flag_and_the_second_lets_the_process_end() {
        let first = catch_ctrl_c("").unwrap();
        let second = catch_ctrl_c("").unwrap();
        let flag = first.flag();
        assert!(!flag.load(Ordering::SeqCst));

        assert!(on_press(), "the first press is caught");
        assert!(flag.load(Ordering::SeqCst));
        assert!(!on_press(), "the second press ends the process");

        drop(second);
        drop(first);
        // A new guard starts over, so a press from before doesn't count.
        let again = catch_ctrl_c("").unwrap();
        assert!(!again.flag().load(Ordering::SeqCst));
    }
}
