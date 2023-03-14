#![allow(non_snake_case, non_camel_case_types)]

use std::fs::File;
use std::io::{Error, Result};

use std::ffi::OsStr;
use std::iter;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::ptr;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winapi;
use winapi::shared::minwindef::{BOOL, DWORD, LPVOID};
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::minwinbase::{LPSECURITY_ATTRIBUTES, SECURITY_ATTRIBUTES};
use winapi::um::processthreadsapi::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW};
use winapi::um::winbase::CREATE_UNICODE_ENVIRONMENT;
use winapi::um::winnt::PHANDLE;
use winapi::um::{handleapi, namedpipeapi, processenv, processthreadsapi, synchapi};

pub use winapi::shared::winerror::{ERROR_ACCESS_DENIED, ERROR_BAD_PATHNAME};
pub const STILL_ACTIVE: u32 = 259;

use crate::os_common::StandardStream;

#[derive(Debug)]
pub struct Handle(RawHandle);

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.as_raw_handle());
        }
    }
}

impl AsRawHandle for Handle {
    fn as_raw_handle(&self) -> RawHandle {
        self.0
    }
}

impl FromRawHandle for Handle {
    unsafe fn from_raw_handle(handle: RawHandle) -> Handle {
        Handle(handle)
    }
}

pub const HANDLE_FLAG_INHERIT: u32 = 1;
pub const STARTF_USESTDHANDLES: DWORD = winapi::um::winbase::STARTF_USESTDHANDLES;

fn check(status: BOOL) -> Result<()> {
    if status != 0 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

fn check_handle(raw_handle: RawHandle) -> Result<RawHandle> {
    if raw_handle != INVALID_HANDLE_VALUE {
        Ok(raw_handle)
    } else {
        Err(Error::last_os_error())
    }
}

// OsStr to zero-terminated owned vector
fn to_nullterm(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(iter::once(0u16)).collect()
}

pub fn CreatePipe(inherit_handle: bool) -> Result<(File, File)> {
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: inherit_handle as BOOL,
    };
    let (mut r, mut w) = (ptr::null_mut(), ptr::null_mut());
    check(unsafe {
        namedpipeapi::CreatePipe(
            &mut r as PHANDLE,
            &mut w as PHANDLE,
            &mut attributes as LPSECURITY_ATTRIBUTES,
            0,
        )
    })?;
    Ok(unsafe { (File::from_raw_handle(r), File::from_raw_handle(w)) })
}

pub fn SetHandleInformation(handle: &File, dwMask: u32, dwFlags: u32) -> Result<()> {
    check(unsafe { handleapi::SetHandleInformation(handle.as_raw_handle(), dwMask, dwFlags) })?;
    Ok(())
}

pub fn CreateProcess(
    appname: Option<&OsStr>,
    cmdline: &OsStr,
    env_block: &Option<Vec<u16>>,
    cwd: &Option<&OsStr>,
    inherit_handles: bool,
    mut creation_flags: u32,
    stdin: Option<RawHandle>,
    stdout: Option<RawHandle>,
    stderr: Option<RawHandle>,
    sinfo_flags: u32,
) -> Result<(Handle, u64)> {
    let mut sinfo: STARTUPINFOW = unsafe { mem::zeroed() };
    sinfo.cb = mem::size_of::<STARTUPINFOW>() as DWORD;
    sinfo.hStdInput = stdin.unwrap_or(ptr::null_mut());
    sinfo.hStdOutput = stdout.unwrap_or(ptr::null_mut());
    sinfo.hStdError = stderr.unwrap_or(ptr::null_mut());
    sinfo.dwFlags = sinfo_flags;
    let mut pinfo: PROCESS_INFORMATION = unsafe { mem::zeroed() };
    let mut cmdline = to_nullterm(cmdline);
    let wc_appname = appname.map(to_nullterm);
    let env_block_ptr = env_block
        .as_ref()
        .map(|v| v.as_ptr())
        .unwrap_or(ptr::null()) as LPVOID;
    let cwd = cwd.map(to_nullterm);
    creation_flags |= CREATE_UNICODE_ENVIRONMENT;
    check(unsafe {
        CreateProcessW(
            wc_appname
                .as_ref()
                .map(|v| v.as_ptr())
                .unwrap_or(ptr::null()),
            cmdline.as_mut_ptr(),
            ptr::null_mut(),         // lpProcessAttributes
            ptr::null_mut(),         // lpThreadAttributes
            inherit_handles as BOOL, // bInheritHandles
            creation_flags,          // dwCreationFlags
            env_block_ptr,           // lpEnvironment
            cwd.as_ref().map(|v| v.as_ptr()).unwrap_or(ptr::null()), // lpCurrentDirectory
            &mut sinfo,
            &mut pinfo,
        )
    })?;
    unsafe {
        drop(Handle::from_raw_handle(pinfo.hThread));
        Ok((
            Handle::from_raw_handle(pinfo.hProcess),
            pinfo.dwProcessId as u64,
        ))
    }
}

pub enum WaitEvent {
    OBJECT_0,
    ABANDONED,
    TIMEOUT,
}

pub fn WaitForSingleObject(handle: &Handle, mut timeout: Option<Duration>) -> Result<WaitEvent> {
    use winapi::shared::winerror::WAIT_TIMEOUT;
    use winapi::um::winbase::{INFINITE, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0};
    let deadline = timeout.map(|timeout| Instant::now() + timeout);

    let result = loop {
        // Allow timeouts greater than 50 days by clamping the
        // timeout and sleeping in a loop.
        let (timeout_ms, overflow) = timeout
            .map(|timeout| {
                let timeout = timeout.as_millis();
                if timeout < INFINITE as u128 {
                    (timeout as u32, false)
                } else {
                    (INFINITE - 1, true)
                }
            })
            .unwrap_or((INFINITE, false));

        let result = unsafe { synchapi::WaitForSingleObject(handle.as_raw_handle(), timeout_ms) };
        if result != WAIT_TIMEOUT || !overflow {
            break result;
        }
        let deadline = deadline.unwrap();
        let now = Instant::now();
        if now >= deadline {
            break WAIT_TIMEOUT;
        }
        timeout = Some(deadline - now);
    };

    if result == WAIT_OBJECT_0 {
        Ok(WaitEvent::OBJECT_0)
    } else if result == WAIT_ABANDONED {
        Ok(WaitEvent::ABANDONED)
    } else if result == WAIT_TIMEOUT {
        Ok(WaitEvent::TIMEOUT)
    } else if result == WAIT_FAILED {
        Err(Error::last_os_error())
    } else {
        panic!(format!("WaitForSingleObject returned {}", result));
    }
}

pub fn GetExitCodeProcess(handle: &Handle) -> Result<u32> {
    let mut exit_code = 0u32;
    check(unsafe {
        processthreadsapi::GetExitCodeProcess(handle.as_raw_handle(), &mut exit_code as *mut u32)
    })?;
    Ok(exit_code)
}

pub fn TerminateProcess(handle: &Handle, exit_code: u32) -> Result<()> {
    check(unsafe { processthreadsapi::TerminateProcess(handle.as_raw_handle(), exit_code) })
}

unsafe fn GetStdHandle(which: StandardStream) -> Result<RawHandle> {
    // private/unsafe because the raw handle it returns must be
    // duplicated or leaked before converting to an owned Handle.
    use winapi::um::winbase::{STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    let id = match which {
        StandardStream::Input => STD_INPUT_HANDLE,
        StandardStream::Output => STD_OUTPUT_HANDLE,
        StandardStream::Error => STD_ERROR_HANDLE,
    };
    let raw_handle = check_handle(processenv::GetStdHandle(id))?;
    Ok(raw_handle)
}

pub fn make_standard_stream(which: StandardStream) -> Result<Rc<File>> {
    unsafe {
        let raw = GetStdHandle(which)?;
        let stream = Rc::new(File::from_raw_handle(raw));
        // Leak the Rc so the object we return doesn't close the underlying
        // system handle.
        mem::forget(Rc::clone(&stream));
        Ok(stream)
    }
}
