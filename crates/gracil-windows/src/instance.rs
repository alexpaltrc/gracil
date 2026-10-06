//! One Gracil per user session (spec §52, ADR 0019).
//!
//! The first Gracil process owns a named mutex and serves a named pipe. Every later invocation,
//! a second launch or File Explorer, finds the mutex taken and sends its arguments through the
//! pipe instead, then exits. The running Gracil adds them to its own list.
//!
//! The pipe accepts local connections only and keeps Windows' default permissions, under which only
//! the same user (and administrators) can write to it. What arrives is still treated as untrusted:
//! it is only a list of arguments, parsed like a command line.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem::ManuallyDrop;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::FromRawHandle;
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED,
    GetLastError, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT, WaitNamedPipeW,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId, OpenMutexW};
use windows_sys::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};

use crate::wide;

/// The mutex's name. `Local\` scopes it to the user's session.
const MUTEX: &str = r"Local\Gracil.SingleInstance";

/// The most one request may carry; far beyond any real selection of videos.
const MAX_REQUEST: u64 = 16 * 1024 * 1024;

/// How long a later invocation keeps trying to reach a Gracil that is still starting up.
const PATIENCE: Duration = Duration::from_secs(10);

/// Tries to become the one Gracil of this session. `true` means this process is it and must call
/// [`serve`]; `false` means another Gracil is running and the request goes to it with
/// [`forward`].
pub fn claim() -> io::Result<bool> {
    let name = wide(MUTEX);
    // SAFETY: the name is a NUL-terminated UTF-16 buffer that outlives the call; no security
    // attributes. The result is checked before use.
    #[allow(unsafe_code)]
    let mutex = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
    if mutex.is_null() {
        return Err(io::Error::last_os_error());
    }
    if last_error() == ERROR_ALREADY_EXISTS {
        // SAFETY: the handle was just opened and is closed once, here.
        #[allow(unsafe_code)]
        unsafe {
            CloseHandle(mutex)
        };
        return Ok(false);
    }
    // The handle is kept for the life of the process; Windows releases it when the process ends,
    // however it ends, so a crashed Gracil never blocks the next one.
    Ok(true)
}

/// Whether a Gracil is running in this session, without becoming it. For File Explorer, which must
/// decide between starting Gracil and handing it the request.
pub fn is_running() -> bool {
    /// The right to wait on the mutex, which is all opening it needs.
    const SYNCHRONIZE: u32 = 0x0010_0000;
    let name = wide(MUTEX);
    // SAFETY: the name is a NUL-terminated UTF-16 buffer that outlives the call; the handle is
    // checked, and closed once if it was opened.
    #[allow(unsafe_code)]
    unsafe {
        let mutex = OpenMutexW(SYNCHRONIZE, 0, name.as_ptr());
        if mutex.is_null() {
            return false;
        }
        CloseHandle(mutex);
    }
    true
}

/// Serves the pipe on a thread of its own, calling `on_request` with each request's arguments.
/// Fails if the pipe can't be created, for example because something else already holds its name.
pub fn serve(on_request: impl Fn(Vec<OsString>) + Send + 'static) -> io::Result<()> {
    serve_on(&pipe_name()?, on_request)
}

fn serve_on(name: &str, on_request: impl Fn(Vec<OsString>) + Send + 'static) -> io::Result<()> {
    let name = wide(name);
    // SAFETY: the name is a NUL-terminated UTF-16 buffer that outlives the call; no security
    // attributes, so Windows' defaults apply. The result is checked before use.
    #[allow(unsafe_code)]
    let pipe = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            64 * 1024,
            0,
            ptr::null(),
        )
    };
    if pipe == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // The handle lives as long as the process, owned by this thread.
    let pipe = pipe as usize;
    thread::Builder::new()
        .name("gracil-requests".into())
        .spawn(move || {
            let pipe = pipe as *mut core::ffi::c_void;
            loop {
                // SAFETY: the pipe handle stays open for the life of the process; no overlapped
                // I/O, so the call waits for a client.
                #[allow(unsafe_code)]
                let connected = unsafe { ConnectNamedPipe(pipe, ptr::null_mut()) } != 0
                    // A client that connected first is just as welcome, even one that already
                    // wrote its request and left: what it wrote is still waiting to be read.
                    || matches!(last_error(), ERROR_PIPE_CONNECTED | ERROR_NO_DATA);
                if connected {
                    // SAFETY: the handle is valid; ManuallyDrop keeps `File` from closing it, as
                    // it is reused for the next client.
                    #[allow(unsafe_code)]
                    let mut file = ManuallyDrop::new(unsafe { File::from_raw_handle(pipe) });
                    let mut bytes = Vec::new();
                    // A client that goes away early just sends less.
                    let _ = (&mut *file).take(MAX_REQUEST).read_to_end(&mut bytes);
                    // No arguments is a plain launch, from Start or the taskbar: it still asks
                    // for the window.
                    on_request(decode(&bytes));
                }
                // SAFETY: the handle is valid; disconnecting readies it for the next client.
                #[allow(unsafe_code)]
                unsafe {
                    DisconnectNamedPipe(pipe)
                };
            }
        })?;
    Ok(())
}

/// Sends `arguments` to the running Gracil. Waits for it while it is still starting, up to a few
/// seconds; an error means it could not be reached.
pub fn forward(arguments: &[OsString]) -> io::Result<()> {
    forward_to(&pipe_name()?, arguments)
}

fn forward_to(name: &str, arguments: &[OsString]) -> io::Result<()> {
    let message = encode(arguments);
    // Lets the running Gracil bring its window to the front when the request asks for it.
    // SAFETY: plain call; failing only means the window may not come forward.
    #[allow(unsafe_code)]
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY)
    };
    let deadline = Instant::now() + PATIENCE;
    loop {
        match OpenOptions::new().write(true).open(name) {
            Ok(mut pipe) => return pipe.write_all(&message),
            Err(error) if Instant::now() >= deadline => return Err(error),
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                let wide_name = wide(name);
                // SAFETY: the name is a NUL-terminated UTF-16 buffer that outlives the call.
                #[allow(unsafe_code)]
                unsafe {
                    WaitNamedPipeW(wide_name.as_ptr(), 1_000)
                };
            }
            // Not there yet: the running Gracil is still starting.
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// The pipe's name. Pipes are visible across sessions, so the session is part of it.
fn pipe_name() -> io::Result<String> {
    let mut session = 0;
    // SAFETY: `session` is a valid place for the answer.
    #[allow(unsafe_code)]
    let found = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) };
    if found == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(format!(r"\\.\pipe\Gracil.{session}"))
}

/// Arguments as UTF-16 (little-endian), each followed by a NUL. Paths survive whatever characters
/// they hold.
fn encode(arguments: &[OsString]) -> Vec<u8> {
    arguments
        .iter()
        .flat_map(|argument| argument.encode_wide().chain([0]))
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// The reverse of [`encode`]. Empty arguments, and an unfinished last one, are dropped.
fn decode(bytes: &[u8]) -> Vec<OsString> {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .collect();
    let mut parts: Vec<&[u16]> = units.split(|&unit| unit == 0).collect();
    // What follows the last NUL was cut short.
    parts.pop();
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(OsString::from_wide)
        .collect()
}

/// The error code the last Windows call on this thread left; read it right after that call.
fn last_error() -> u32 {
    // SAFETY: takes no arguments and only reads this thread's last error.
    #[allow(unsafe_code)]
    unsafe {
        GetLastError()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn requests_reach_the_running_gracil_in_order() {
        use std::sync::mpsc;

        // A name of its own, so a Gracil running on this computer is not disturbed.
        let name = format!(r"\\.\pipe\Gracil.Test.{}", std::process::id());
        let (sent, received) = mpsc::channel();
        serve_on(&name, move |arguments| sent.send(arguments).unwrap()).unwrap();

        let first = [OsString::from(r"C:\Vídeos\uno.mov")];
        let second = [OsString::from("--save-copy"), OsString::from(r"D:\dos.mkv")];
        forward_to(&name, &first).unwrap();
        forward_to(&name, &second).unwrap();
        forward_to(&name, &[]).unwrap();

        let wait = Duration::from_secs(5);
        assert_eq!(received.recv_timeout(wait).unwrap(), first);
        assert_eq!(received.recv_timeout(wait).unwrap(), second);
        // A plain launch sends nothing, and is still heard.
        assert!(received.recv_timeout(wait).unwrap().is_empty());
        // Only one server may hold the name.
        assert!(serve_on(&name, |_| {}).is_err());
    }

    #[test]
    fn arguments_round_trip() {
        let arguments: Vec<OsString> = [
            "--replace",
            r"C:\Vídeos\Cumpleaños 🎂.mov",
            r"\\server\share\clip.mp4",
        ]
        .map(OsString::from)
        .into();
        assert_eq!(decode(&encode(&arguments)), arguments);
    }

    #[test]
    fn unpaired_surrogates_survive() {
        // Windows file names may hold UTF-16 that is not valid Unicode.
        let odd = OsString::from_wide(&[0x0043, 0xD800, 0x0078]);
        assert_eq!(decode(&encode(std::slice::from_ref(&odd))), [odd]);
    }

    #[test]
    fn a_cut_short_request_keeps_only_whole_arguments() {
        let bytes = encode(&[OsString::from("a.mp4"), OsString::from("b.mp4")]);
        assert_eq!(decode(&bytes[..bytes.len() - 3]), [OsString::from("a.mp4")]);
        assert!(decode(&[]).is_empty());
        assert!(decode(&[0, 0, 0, 0]).is_empty());
    }
}
