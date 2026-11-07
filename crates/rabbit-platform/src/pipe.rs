//! The named pipe between RABBIT and its elevated uninstall helper.
//!
//! RABBIT creates the pipe ([`PipeServer`]) before it starts the helper,
//! and passes the pipe's name and its own process ID on the helper's
//! command line. The helper connects ([`PipeChannel::connect`]). Each side
//! checks the other's process ID, as Windows reports it, before trusting a
//! single line: RABBIT accepts only the helper it started, and the helper
//! talks only to the RABBIT that started it. A wrong process ID is a hard
//! error that closes the pipe.
//!
//! The channel carries lines of UTF-8 text, one message per line; what a
//! line means is the caller's business. Every wait has a time limit, so
//! neither side can hang on the other.
//!
//! The pipe accepts one client, never one from another computer, and only
//! SYSTEM, the Administrators group and the account RABBIT runs as may open
//! it. The helper opens it at identification level, so RABBIT can't act
//! with the helper's rights through it.
//!
//! Windows only; elsewhere creating or connecting fails as `Unsupported`.

use std::io;
use std::time::Duration;

/// Every uninstall pipe name starts with this.
pub const UNINSTALL_PIPE_PREFIX: &str = "\\\\.\\pipe\\rabbit-uninstall-";

/// The longest line either side accepts, so a confused peer can't make the
/// other buffer without end.
pub const MAX_PIPE_LINE_BYTES: usize = 1024 * 1024;

/// A new, unguessable pipe name:
/// `\\.\pipe\rabbit-uninstall-<this process's ID>-<128 random bits in hex>`,
/// from the OS random source.
pub fn new_uninstall_pipe_name() -> io::Result<String> {
    imp::new_uninstall_pipe_name()
}

/// The pipe name for process `pid` and these random bytes.
pub fn uninstall_pipe_name(pid: u32, random: &[u8; 16]) -> String {
    let random: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{UNINSTALL_PIPE_PREFIX}{pid}-{random}")
}

/// Whether `name` has exactly the form [`new_uninstall_pipe_name`] makes,
/// so the helper can refuse any other name on its command line.
pub fn is_uninstall_pipe_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(UNINSTALL_PIPE_PREFIX) else {
        return false;
    };
    let Some((pid, random)) = rest.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && pid.len() <= 10
        && pid.bytes().all(|byte| byte.is_ascii_digit())
        && pid.parse::<u32>().is_ok()
        && random.len() == 32
        && random
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// The server end, created by the normal, non-elevated RABBIT. It accepts
/// exactly one client.
#[derive(Debug)]
pub struct PipeServer {
    inner: imp::Server,
    name: String,
}

impl PipeServer {
    /// Create the pipe `name`, which must start with `\\.\pipe\`. Fails if
    /// a pipe of that name already exists, whoever made it.
    pub fn create(name: &str) -> io::Result<Self> {
        check_pipe_name(name)?;
        Ok(Self {
            inner: imp::Server::create(name)?,
            name: name.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Wait up to `timeout` for the client to connect, then check that it
    /// is process `expected_client_pid`. Anything else closes the pipe:
    /// `TimedOut` when nobody connected in time, `PermissionDenied` when
    /// another process did.
    pub fn accept(self, expected_client_pid: u32, timeout: Duration) -> io::Result<PipeChannel> {
        let (pipe, peer_pid) = self.inner.accept(expected_client_pid, timeout)?;
        Ok(PipeChannel::new(pipe, peer_pid))
    }
}

/// A connected pipe, from either end. Dropping it closes the pipe; the
/// other side then reads the end of the stream.
#[derive(Debug)]
pub struct PipeChannel {
    pipe: imp::Connected,
    peer_pid: u32,
    pending: Vec<u8>,
    closed: bool,
}

impl PipeChannel {
    /// Connect to the pipe `name`, waiting up to `timeout` for it to exist
    /// and be free, then check that the process serving it is
    /// `expected_server_pid`. `TimedOut` when it never became available,
    /// `PermissionDenied` (and the pipe closed) when another process
    /// serves it.
    pub fn connect(name: &str, expected_server_pid: u32, timeout: Duration) -> io::Result<Self> {
        check_pipe_name(name)?;
        let (pipe, peer_pid) = imp::Connected::connect(name, expected_server_pid, timeout)?;
        Ok(Self::new(pipe, peer_pid))
    }

    fn new(pipe: imp::Connected, peer_pid: u32) -> Self {
        Self {
            pipe,
            peer_pid,
            pending: Vec::new(),
            closed: false,
        }
    }

    /// The process ID of the other end, as checked when connecting.
    pub fn peer_pid(&self) -> u32 {
        self.peer_pid
    }

    /// Send `line` and a line break, waiting up to `timeout` for the other
    /// side to take it. `line` must not contain a line break itself.
    ///
    /// After an error the other side may have received part of the line,
    /// so the channel can't be trusted any more: drop it.
    pub fn send_line(&mut self, line: &str, timeout: Duration) -> io::Result<()> {
        if line.contains(['\n', '\r']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a pipe message must fit on one line",
            ));
        }
        let mut bytes = Vec::with_capacity(line.len() + 1);
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
        self.pipe.write_all(&bytes, timeout)
    }

    /// The next line, without its line break, waiting up to `timeout` for
    /// it. `Ok(None)` once the other side has closed the pipe and every
    /// line has been read.
    ///
    /// `TimedOut` when no whole line arrived in time; nothing is lost, and
    /// the next call goes on where this one stopped. A line that isn't
    /// UTF-8 or is longer than [`MAX_PIPE_LINE_BYTES`] is `InvalidData`, and a
    /// pipe closed in the middle of a line is `UnexpectedEof`.
    pub fn read_line(&mut self, timeout: Duration) -> io::Result<Option<String>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
                let mut line: Vec<u8> = self.pending.drain(..=end).collect();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return String::from_utf8(line).map(Some).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "a pipe message isn't UTF-8")
                });
            }
            if self.pending.len() > MAX_PIPE_LINE_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "a pipe message is too long",
                ));
            }
            if self.closed {
                if self.pending.is_empty() {
                    return Ok(None);
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the pipe closed in the middle of a message",
                ));
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match self.pipe.read_some(remaining)? {
                Some(bytes) => self.pending.extend_from_slice(&bytes),
                None => self.closed = true,
            }
        }
    }
}

fn check_pipe_name(name: &str) -> io::Result<()> {
    let prefix = "\\\\.\\pipe\\";
    let has_prefix = name
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix));
    if !has_prefix || name.len() == prefix.len() || name.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name:?} isn't a local pipe name"),
        ));
    }
    Ok(())
}

#[cfg(windows)]
mod imp {
    use std::fs::OpenOptions;
    use std::io;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{
        ERROR_BROKEN_PIPE, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING, ERROR_OPERATION_ABORTED,
        ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED, GetLastError, HANDLE,
        INVALID_HANDLE_VALUE, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX, ReadFile,
        SECURITY_IDENTIFICATION, WriteFile,
    };
    use windows_sys::Win32::System::IO::{
        CancelIoEx, GetOverlappedResult, GetOverlappedResultEx, OVERLAPPED,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
        GetNamedPipeServerProcessId, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_WAIT, WaitNamedPipeW,
    };
    use windows_sys::Win32::System::Threading::CreateEventW;

    use crate::win_util::{current_user_sid, random_hex, security_descriptor_from_sddl, wide_str};

    /// Bytes Windows buffers each way. Lines are short; this is plenty.
    const BUFFER_BYTES: u32 = 64 * 1024;

    /// How long a client sleeps between tries while the pipe doesn't exist
    /// yet, and the longest it waits for a busy one at a time.
    const CONNECT_RETRY: Duration = Duration::from_millis(50);

    pub(super) fn new_uninstall_pipe_name() -> io::Result<String> {
        let random = random_hex::<16>()?;
        Ok(format!(
            "{}{}-{random}",
            super::UNINSTALL_PIPE_PREFIX,
            std::process::id()
        ))
    }

    #[derive(Debug)]
    pub(super) struct Server {
        pipe: OwnedHandle,
    }

    impl Server {
        #[cfg(test)]
        pub(super) fn raw_handle(&self) -> HANDLE {
            self.pipe.as_raw_handle()
        }

        pub(super) fn create(name: &str) -> io::Result<Self> {
            // Only SYSTEM, Administrators (the elevated helper, whichever
            // account it runs as) and this account. Windows' default would
            // also let everyone read.
            let sddl = format!(
                "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;{})",
                current_user_sid()?
            );
            let descriptor = security_descriptor_from_sddl(&sddl)?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.as_ptr(),
                bInheritHandle: 0,
            };
            let wide = wide_str(std::ffi::OsStr::new(name))?;
            // SAFETY: `wide` is NUL-terminated and `attributes` points to a
            // descriptor that outlives the call.
            let handle = unsafe {
                CreateNamedPipeW(
                    wide.as_ptr(),
                    PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    1,
                    BUFFER_BYTES,
                    BUFFER_BYTES,
                    0,
                    &attributes,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: a valid handle, owned by nobody else.
            Ok(Self {
                pipe: unsafe { OwnedHandle::from_raw_handle(handle) },
            })
        }

        pub(super) fn accept(
            self,
            expected_client_pid: u32,
            timeout: Duration,
        ) -> io::Result<(Connected, u32)> {
            let pipe = self.pipe.as_raw_handle();
            let event = new_event()?;
            let mut overlapped = OVERLAPPED {
                hEvent: event.as_raw_handle(),
                ..Default::default()
            };
            // SAFETY: `overlapped` and its event outlive the operation: it
            // is waited for, or cancelled and waited for, below.
            if unsafe { ConnectNamedPipe(pipe, &mut overlapped) } == 0 {
                // SAFETY: read right after the failed call.
                match unsafe { GetLastError() } {
                    // The client was quicker than this call.
                    ERROR_PIPE_CONNECTED => {}
                    ERROR_IO_PENDING => {
                        wait_overlapped(pipe, &overlapped, timeout, "nobody connected")?;
                    }
                    code => return Err(io::Error::from_raw_os_error(code as i32)),
                }
            }

            let mut client_pid = 0u32;
            // SAFETY: the pipe is connected and `client_pid` is a valid
            // place for the ID.
            if unsafe { GetNamedPipeClientProcessId(pipe, &mut client_pid) } == 0 {
                let error = io::Error::last_os_error();
                disconnect(pipe);
                return Err(error);
            }
            if client_pid != expected_client_pid {
                disconnect(pipe);
                return Err(wrong_peer("client", client_pid, expected_client_pid));
            }
            Ok((Connected::new(self.pipe)?, client_pid))
        }
    }

    #[derive(Debug)]
    pub(super) struct Connected {
        pipe: OwnedHandle,
        read_event: OwnedHandle,
        write_event: OwnedHandle,
    }

    impl Connected {
        fn new(pipe: OwnedHandle) -> io::Result<Self> {
            Ok(Self {
                pipe,
                read_event: new_event()?,
                write_event: new_event()?,
            })
        }

        #[cfg(test)]
        pub(super) fn raw_handle(&self) -> HANDLE {
            self.pipe.as_raw_handle()
        }

        pub(super) fn connect(
            name: &str,
            expected_server_pid: u32,
            timeout: Duration,
        ) -> io::Result<(Self, u32)> {
            let deadline = Instant::now() + timeout;
            let file = loop {
                // Identification level: the server may learn who the
                // helper is, but can never act with its rights.
                let opened = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(FILE_FLAG_OVERLAPPED)
                    .security_qos_flags(SECURITY_IDENTIFICATION)
                    .open(name);
                let error = match opened {
                    Ok(file) => break file,
                    Err(error) => error,
                };
                let busy = error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32);
                if !busy && error.kind() != io::ErrorKind::NotFound {
                    return Err(error);
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("the pipe {name} didn't become available in time"),
                    ));
                }
                let pause = remaining.min(CONNECT_RETRY);
                if busy {
                    let wide = wide_str(std::ffi::OsStr::new(name))?;
                    // SAFETY: `wide` is NUL-terminated. The result doesn't
                    // matter: the open above is tried again either way.
                    // 0 would mean the pipe's default wait, so at least 1.
                    unsafe { WaitNamedPipeW(wide.as_ptr(), (pause.as_millis() as u32).max(1)) };
                } else {
                    std::thread::sleep(pause);
                }
            };
            let pipe = OwnedHandle::from(file);

            let mut server_pid = 0u32;
            // SAFETY: the pipe is open and `server_pid` is a valid place
            // for the ID.
            if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut server_pid) } == 0 {
                return Err(io::Error::last_os_error());
            }
            if server_pid != expected_server_pid {
                return Err(wrong_peer("server", server_pid, expected_server_pid));
            }
            Ok((Self::new(pipe)?, server_pid))
        }

        /// Some bytes, waiting up to `timeout` for at least one; `None`
        /// once the other side has closed the pipe.
        pub(super) fn read_some(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            let pipe = self.pipe.as_raw_handle();
            let mut buffer = vec![0u8; 4096];
            let mut overlapped = OVERLAPPED {
                hEvent: self.read_event.as_raw_handle(),
                ..Default::default()
            };
            // SAFETY: `buffer` and `overlapped` outlive the operation: it is
            // waited for, or cancelled and waited for, before they drop.
            let ok = unsafe {
                ReadFile(
                    pipe,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    std::ptr::null_mut(),
                    &mut overlapped,
                )
            };
            if ok == 0 {
                // SAFETY: read right after the failed call.
                let code = unsafe { GetLastError() };
                if is_closed(code) {
                    return Ok(None);
                }
                if code != ERROR_IO_PENDING {
                    return Err(io::Error::from_raw_os_error(code as i32));
                }
            }
            match wait_overlapped(pipe, &overlapped, timeout, "no whole message arrived") {
                Ok(read) => {
                    buffer.truncate(read as usize);
                    Ok(Some(buffer))
                }
                Err(error)
                    if error
                        .raw_os_error()
                        .is_some_and(|code| is_closed(code as u32)) =>
                {
                    Ok(None)
                }
                Err(error) => Err(error),
            }
        }

        pub(super) fn write_all(&mut self, bytes: &[u8], timeout: Duration) -> io::Result<()> {
            let pipe = self.pipe.as_raw_handle();
            let deadline = Instant::now() + timeout;
            let mut written = 0usize;
            while written < bytes.len() {
                let chunk = &bytes[written..];
                let length = chunk.len().min(u32::MAX as usize) as u32;
                let mut overlapped = OVERLAPPED {
                    hEvent: self.write_event.as_raw_handle(),
                    ..Default::default()
                };
                // SAFETY: `bytes` and `overlapped` outlive the operation, as
                // in `read_some`.
                let ok = unsafe {
                    WriteFile(
                        pipe,
                        chunk.as_ptr(),
                        length,
                        std::ptr::null_mut(),
                        &mut overlapped,
                    )
                };
                if ok == 0 {
                    // SAFETY: read right after the failed call.
                    let code = unsafe { GetLastError() };
                    if code != ERROR_IO_PENDING {
                        return Err(io::Error::from_raw_os_error(code as i32));
                    }
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                written +=
                    wait_overlapped(pipe, &overlapped, remaining, "the other side didn't read")?
                        as usize;
            }
            Ok(())
        }
    }

    fn is_closed(code: u32) -> bool {
        code == ERROR_BROKEN_PIPE || code == ERROR_PIPE_NOT_CONNECTED
    }

    /// Wait up to `timeout` for an overlapped operation and return the
    /// bytes it moved. On timeout, cancel it and wait for the cancel, so
    /// Windows is done with the buffer and `overlapped` before the caller
    /// drops them; an operation that finished just before the cancel still
    /// counts.
    fn wait_overlapped(
        handle: HANDLE,
        overlapped: &OVERLAPPED,
        timeout: Duration,
        what_timed_out: &str,
    ) -> io::Result<u32> {
        let milliseconds = u32::try_from(timeout.as_millis())
            .unwrap_or(u32::MAX - 1)
            .min(u32::MAX - 1);
        let mut transferred = 0u32;
        // SAFETY: `overlapped` belongs to an operation started on `handle`.
        if unsafe { GetOverlappedResultEx(handle, overlapped, &mut transferred, milliseconds, 0) }
            != 0
        {
            return Ok(transferred);
        }
        // SAFETY: read right after the failed call.
        let code = unsafe { GetLastError() };
        if code != WAIT_TIMEOUT && code != ERROR_IO_INCOMPLETE {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        // SAFETY: as above; the wait that follows lets the cancel finish.
        unsafe { CancelIoEx(handle, overlapped) };
        if unsafe { GetOverlappedResult(handle, overlapped, &mut transferred, 1) } != 0 {
            return Ok(transferred);
        }
        // SAFETY: read right after the failed call.
        let code = unsafe { GetLastError() };
        if code == ERROR_OPERATION_ABORTED {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("{what_timed_out} in time"),
            ));
        }
        Err(io::Error::from_raw_os_error(code as i32))
    }

    fn new_event() -> io::Result<OwnedHandle> {
        // Manual reset, as overlapped I/O needs.
        // SAFETY: no attributes and no name; the handle is checked below.
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a valid handle, owned by nobody else.
        Ok(unsafe { OwnedHandle::from_raw_handle(event) })
    }

    fn disconnect(pipe: HANDLE) {
        // SAFETY: `pipe` is a server pipe handle; a failure leaves it to be
        // closed when its owner drops.
        unsafe { DisconnectNamedPipe(pipe) };
    }

    fn wrong_peer(side: &str, actual: u32, expected: u32) -> io::Error {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("the pipe's {side} is process {actual}, not process {expected}"),
        )
    }
}

#[cfg(not(windows))]
mod imp {
    use std::io;
    use std::time::Duration;

    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "the uninstall helper's pipe only exists on Windows",
        )
    }

    pub(super) fn new_uninstall_pipe_name() -> io::Result<String> {
        Err(unsupported())
    }

    /// Never made off Windows.
    #[derive(Debug)]
    pub(super) struct Server {
        never: std::convert::Infallible,
    }

    impl Server {
        pub(super) fn create(_name: &str) -> io::Result<Self> {
            Err(unsupported())
        }

        pub(super) fn accept(
            self,
            _expected_client_pid: u32,
            _timeout: Duration,
        ) -> io::Result<(Connected, u32)> {
            match self.never {}
        }
    }

    /// Never made off Windows.
    #[derive(Debug)]
    pub(super) struct Connected {
        never: std::convert::Infallible,
    }

    impl Connected {
        pub(super) fn connect(
            _name: &str,
            _expected_server_pid: u32,
            _timeout: Duration,
        ) -> io::Result<(Self, u32)> {
            Err(unsupported())
        }

        pub(super) fn read_some(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            match self.never {}
        }

        pub(super) fn write_all(&mut self, _bytes: &[u8], _timeout: Duration) -> io::Result<()> {
            match self.never {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_names_carry_the_process_id_and_128_random_bits() {
        let name = uninstall_pipe_name(4242, &[0xab; 16]);
        assert_eq!(
            name,
            "\\\\.\\pipe\\rabbit-uninstall-4242-abababababababababababababababab"
        );
        assert!(is_uninstall_pipe_name(&name));
    }

    #[test]
    fn only_names_of_exactly_that_form_are_uninstall_pipe_names() {
        for name in [
            "\\\\.\\pipe\\rabbit-uninstall-4242-ABABABABABABABABABABABABABABABAB",
            "\\\\.\\pipe\\rabbit-uninstall-4242-abab",
            "\\\\.\\pipe\\rabbit-uninstall--abababababababababababababababab",
            "\\\\.\\pipe\\rabbit-uninstall-99999999999-abababababababababababababababab",
            "\\\\.\\pipe\\rabbit-uninstall-4242-abababababababababababababababab-x",
            "\\\\server\\pipe\\rabbit-uninstall-4242-abababababababababababababababab",
            "\\\\.\\pipe\\other-4242-abababababababababababababababab",
            "",
        ] {
            assert!(!is_uninstall_pipe_name(name), "{name}");
        }
    }

    #[test]
    fn only_local_pipe_names_are_used() {
        for name in [
            "",
            "\\\\.\\pipe\\",
            "C:\\pipe\\x",
            "\\\\server\\pipe\\x",
            "\\\\.\\pipe\\a\0b",
        ] {
            assert_eq!(
                PipeServer::create(name).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{name:?}"
            );
            assert_eq!(
                PipeChannel::connect(name, 1, Duration::ZERO)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput,
                "{name:?}"
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_there_is_no_pipe() {
        assert_eq!(
            new_uninstall_pipe_name().unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        let name = uninstall_pipe_name(1, &[0; 16]);
        assert_eq!(
            PipeServer::create(&name).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            PipeChannel::connect(&name, 1, Duration::ZERO)
                .unwrap_err()
                .kind(),
            io::ErrorKind::Unsupported
        );
    }
}

/// Both ends live in this one test process, so the client's and the
/// server's process ID are both this process's.
#[cfg(all(test, windows))]
mod windows_tests {
    use std::thread;
    use std::time::Instant;

    use super::*;

    const LONG: Duration = Duration::from_secs(20);

    fn this_process() -> u32 {
        std::process::id()
    }

    /// Never a real Windows process ID, which are multiples of four.
    fn another_process() -> u32 {
        this_process() + 1
    }

    #[test]
    fn new_names_are_well_formed_and_never_repeat() {
        let first = new_uninstall_pipe_name().unwrap();
        let second = new_uninstall_pipe_name().unwrap();
        assert!(is_uninstall_pipe_name(&first), "{first}");
        assert!(
            first.starts_with(&format!("{UNINSTALL_PIPE_PREFIX}{}-", this_process())),
            "{first}"
        );
        assert_ne!(first, second);
    }

    #[test]
    fn lines_go_both_ways_and_the_end_reads_as_none() {
        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let name = server.name().to_string();
        let helper = thread::spawn(move || {
            let mut channel = PipeChannel::connect(&name, this_process(), LONG).unwrap();
            assert_eq!(channel.peer_pid(), this_process());
            while let Some(order) = channel.read_line(LONG).unwrap() {
                if order == "done" {
                    break;
                }
                channel
                    .send_line(&format!("{{\"done\":{order}}}"), LONG)
                    .unwrap();
            }
            channel.send_line("bye", LONG).unwrap();
        });

        let mut channel = server.accept(this_process(), LONG).unwrap();
        assert_eq!(channel.peer_pid(), this_process());
        let orders = [
            "{\"package\":\"surge-xt\"}",
            "{\"package\":\"reaper\"}",
            "\"ünïcode\"",
        ];
        for order in orders {
            channel.send_line(order, LONG).unwrap();
        }
        for order in orders {
            assert_eq!(
                channel.read_line(LONG).unwrap().as_deref(),
                Some(format!("{{\"done\":{order}}}").as_str())
            );
        }
        channel.send_line("done", LONG).unwrap();
        assert_eq!(channel.read_line(LONG).unwrap().as_deref(), Some("bye"));
        helper.join().unwrap();
        assert_eq!(channel.read_line(LONG).unwrap(), None);
        assert_eq!(channel.read_line(LONG).unwrap(), None);
    }

    #[test]
    fn a_long_line_arrives_whole() {
        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let name = server.name().to_string();
        let long = "x".repeat(300 * 1024);
        let sent = long.clone();
        let helper = thread::spawn(move || {
            let mut channel = PipeChannel::connect(&name, this_process(), LONG).unwrap();
            channel.send_line(&sent, LONG).unwrap();
        });

        let mut channel = server.accept(this_process(), LONG).unwrap();
        assert_eq!(channel.read_line(LONG).unwrap(), Some(long));
        helper.join().unwrap();
    }

    #[test]
    fn a_client_with_the_wrong_process_id_is_refused_and_disconnected() {
        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let name = server.name().to_string();
        let client = thread::spawn(move || {
            PipeChannel::connect(&name, this_process(), LONG)
                .and_then(|mut channel| channel.read_line(LONG))
        });

        let error = server.accept(another_process(), LONG).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        // The client reads the end of the pipe, or, when the server cut it
        // off before the client had checked who serves it, can't even ask
        // (ERROR_PIPE_NOT_CONNECTED).
        match client.join().unwrap() {
            Ok(line) => assert_eq!(line, None),
            Err(error) => assert_eq!(error.raw_os_error(), Some(233), "{error}"),
        }
    }

    #[test]
    fn a_server_with_the_wrong_process_id_is_refused() {
        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let name = server.name().to_string();
        let rabbit = thread::spawn(move || {
            server
                .accept(this_process(), LONG)
                .and_then(|mut channel| channel.read_line(LONG))
        });

        let error = PipeChannel::connect(&name, another_process(), LONG).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        // The client closed the pipe at once: the server either reads the
        // end, or, when the client was gone before it even started waiting,
        // hears "the pipe is being closed" (ERROR_NO_DATA).
        match rabbit.join().unwrap() {
            Ok(line) => assert_eq!(line, None),
            Err(error) => assert_eq!(error.raw_os_error(), Some(232), "{error}"),
        }
    }

    #[test]
    fn connecting_to_a_pipe_nobody_made_times_out() {
        let name = new_uninstall_pipe_name().unwrap();
        let started = Instant::now();

        let error =
            PipeChannel::connect(&name, this_process(), Duration::from_millis(300)).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() >= Duration::from_millis(300));
    }

    #[test]
    fn a_pipe_already_in_use_times_out_for_a_second_client() {
        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let name = server.name().to_string();
        let first = PipeChannel::connect(&name, this_process(), LONG).unwrap();

        let error =
            PipeChannel::connect(&name, this_process(), Duration::from_millis(300)).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        drop(first);
        drop(server);
    }

    #[test]
    fn waiting_for_a_client_that_never_comes_times_out() {
        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let started = Instant::now();

        let error = server
            .accept(this_process(), Duration::from_millis(300))
            .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        // It really waited. accept is one timed wait in Windows, which ends
        // on a clock tick and so can end up to about 16 ms before the time
        // asked for, as the clock behind Instant measures it.
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(250), "{waited:?}");
    }

    #[test]
    fn a_read_that_times_out_loses_nothing() {
        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let name = server.name().to_string();
        let mut client = PipeChannel::connect(&name, this_process(), LONG).unwrap();
        let mut rabbit = server.accept(this_process(), LONG).unwrap();

        let error = rabbit.read_line(Duration::from_millis(100)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        client.send_line("first", LONG).unwrap();
        client.send_line("second", LONG).unwrap();

        assert_eq!(rabbit.read_line(LONG).unwrap().as_deref(), Some("first"));
        assert_eq!(rabbit.read_line(LONG).unwrap().as_deref(), Some("second"));
        let error = rabbit.read_line(Duration::from_millis(100)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn only_system_administrators_and_this_account_may_open_the_pipe() {
        use windows_sys::Win32::Security::Authorization::SE_KERNEL_OBJECT;

        use crate::win_util::test_support::{aces, dacl_sddl_of, sddl_trustee};

        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let dacl = dacl_sddl_of(server.inner.raw_handle(), SE_KERNEL_OBJECT);

        // Protected, so nothing else is added, such as Everyone's read.
        assert!(dacl.starts_with("D:P"), "{dacl}");
        let user = sddl_trustee(&crate::win_util::current_user_sid().unwrap());
        assert_eq!(
            aces(&dacl),
            [
                "A;;FA;;;SY".to_string(),
                "A;;FA;;;BA".to_string(),
                format!("A;;FA;;;{user}"),
            ],
            "{dacl}"
        );
    }

    #[test]
    fn a_second_pipe_of_the_same_name_cannot_be_made() {
        let name = new_uninstall_pipe_name().unwrap();
        let _first = PipeServer::create(&name).unwrap();
        assert!(PipeServer::create(&name).is_err());
    }

    #[test]
    fn a_message_must_fit_on_one_line() {
        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let name = server.name().to_string();
        let mut client = PipeChannel::connect(&name, this_process(), LONG).unwrap();
        for line in ["two\nlines", "carriage\rreturn"] {
            assert_eq!(
                client.send_line(line, LONG).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    /// The elevated helper is the client. If RABBIT, the server, could
    /// impersonate it at a higher level than identification, a compromised
    /// RABBIT could act with the helper's administrator rights.
    #[test]
    fn the_server_can_identify_the_client_but_never_act_as_it() {
        use windows_sys::Win32::Security::{
            GetTokenInformation, RevertToSelf, SECURITY_IMPERSONATION_LEVEL,
            SecurityIdentification, TOKEN_QUERY, TokenImpersonationLevel,
        };
        use windows_sys::Win32::System::Pipes::ImpersonateNamedPipeClient;
        use windows_sys::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};

        /// Ends the impersonation even when an assertion fails.
        struct Revert;
        impl Drop for Revert {
            fn drop(&mut self) {
                // SAFETY: only ever drops on the thread that impersonates.
                unsafe { RevertToSelf() };
            }
        }

        let server = PipeServer::create(&new_uninstall_pipe_name().unwrap()).unwrap();
        let name = server.name().to_string();
        let mut client = PipeChannel::connect(&name, this_process(), LONG).unwrap();
        let mut rabbit = server.accept(this_process(), LONG).unwrap();
        // Windows lets a server impersonate only after reading something.
        client.send_line("hello", LONG).unwrap();
        assert_eq!(rabbit.read_line(LONG).unwrap().as_deref(), Some("hello"));

        // SAFETY: a connected server pipe handle.
        assert_ne!(
            unsafe { ImpersonateNamedPipeClient(rabbit.pipe.raw_handle()) },
            0,
            "{}",
            io::Error::last_os_error()
        );
        let _revert = Revert;
        let mut raw_token = std::ptr::null_mut();
        // SAFETY: the current thread's pseudo handle needs no closing;
        // `raw_token` is a valid place for the token handle.
        assert_ne!(
            unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut raw_token) },
            0,
            "{}",
            io::Error::last_os_error()
        );
        // SAFETY: the call succeeded, so the handle is ours to close.
        let token = unsafe {
            <std::os::windows::io::OwnedHandle as std::os::windows::io::FromRawHandle>::from_raw_handle(
                raw_token,
            )
        };
        let mut level: SECURITY_IMPERSONATION_LEVEL = 0;
        let mut returned = 0u32;
        // SAFETY: `level` is the type TokenImpersonationLevel returns, of
        // the size passed.
        let ok = unsafe {
            GetTokenInformation(
                std::os::windows::io::AsRawHandle::as_raw_handle(&token),
                TokenImpersonationLevel,
                (&raw mut level).cast(),
                std::mem::size_of::<SECURITY_IMPERSONATION_LEVEL>() as u32,
                &mut returned,
            )
        };
        assert_ne!(ok, 0, "{}", io::Error::last_os_error());
        assert_eq!(level, SecurityIdentification);
    }
}
