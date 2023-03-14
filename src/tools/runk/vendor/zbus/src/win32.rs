use std::{
    ffi::CStr,
    io::{Error, ErrorKind},
    net::SocketAddr,
    ptr,
};

use winapi::{
    shared::{
        minwindef::DWORD,
        sddl::ConvertSidToStringSidA,
        tcpmib::{MIB_TCPTABLE2, MIB_TCP_STATE_ESTAB},
        winerror::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR},
        ws2def::INADDR_LOOPBACK,
    },
    um::{
        handleapi::CloseHandle,
        iphlpapi::GetTcpTable2,
        processthreadsapi::{GetCurrentProcess, OpenProcess, OpenProcessToken},
        securitybaseapi::{GetTokenInformation, IsValidSid},
        winbase::LocalFree,
        winnt::{TokenUser, HANDLE, PROCESS_QUERY_LIMITED_INFORMATION, TOKEN_QUERY, TOKEN_USER},
    },
};

#[cfg(feature = "async-io")]
use uds_windows::UnixStream;

// A process handle
pub struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

impl ProcessHandle {
    // Open the process associated with the process_id (if None, the current process)
    pub fn open(process_id: Option<DWORD>, desired_access: DWORD) -> Result<Self, Error> {
        let process = if let Some(process_id) = process_id {
            unsafe { OpenProcess(desired_access, false.into(), process_id) }
        } else {
            unsafe { GetCurrentProcess() }
        };

        if process.is_null() {
            Err(Error::last_os_error())
        } else {
            Ok(Self(process))
        }
    }
}

// A process token
//
// See MSDN documentation:
// https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocesstoken
//
// Get the process security identifier with the `sid()` function.
pub struct ProcessToken(HANDLE);

impl Drop for ProcessToken {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

impl ProcessToken {
    // Open the access token associated with the process_id (if None, the current process)
    pub fn open(process_id: Option<DWORD>) -> Result<Self, Error> {
        let mut process_token: HANDLE = ptr::null_mut();
        let process = ProcessHandle::open(process_id, PROCESS_QUERY_LIMITED_INFORMATION)?;

        if unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut process_token) } == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(Self(process_token))
        }
    }

    // Return the process SID (security identifier) as a string
    pub fn sid(&self) -> Result<String, Error> {
        let mut len = 256;
        let mut token_info;

        loop {
            token_info = vec![0u8; len as usize];

            let result = unsafe {
                GetTokenInformation(
                    self.0,
                    TokenUser,
                    token_info.as_mut_ptr() as *mut _,
                    len,
                    &mut len,
                )
            };

            if result != 0 {
                break;
            }

            let last_error = Error::last_os_error();
            if last_error.raw_os_error() == Some(ERROR_INSUFFICIENT_BUFFER as i32) {
                continue;
            }

            return Err(last_error);
        }

        let sid = unsafe { (*(token_info.as_ptr() as *const TOKEN_USER)).User.Sid };

        if unsafe { IsValidSid(sid as *mut _) } == 0 {
            return Err(Error::new(ErrorKind::Other, "Invalid SID"));
        }

        let mut pstr: *mut i8 = ptr::null_mut();
        if unsafe { ConvertSidToStringSidA(sid as *mut _, &mut pstr as *mut _) } == 0 {
            return Err(Error::last_os_error());
        }

        let sid = unsafe { CStr::from_ptr(pstr) };
        let ret = sid.to_string_lossy();
        unsafe {
            LocalFree(pstr as *mut _);
        }

        Ok(ret.into_owned())
    }
}

// Get the process ID of the local socket address
// TODO: add ipv6 support
pub fn socket_addr_get_pid(addr: &SocketAddr) -> Result<DWORD, Error> {
    let mut len = 4096;
    let mut tcp_table = vec![];
    let res = loop {
        tcp_table.resize(len as usize, 0);
        let res =
            unsafe { GetTcpTable2(tcp_table.as_mut_ptr().cast::<MIB_TCPTABLE2>(), &mut len, 0) };
        if res != ERROR_INSUFFICIENT_BUFFER {
            break res;
        }
    };
    if res != NO_ERROR {
        return Err(Error::last_os_error());
    }

    let tcp_table = tcp_table.as_mut_ptr() as *const MIB_TCPTABLE2;
    let num_entries = unsafe { (*tcp_table).dwNumEntries };
    for i in 0..num_entries {
        let entry = unsafe { (*tcp_table).table.get_unchecked(i as usize) };
        let port = (entry.dwLocalPort & 0xFFFF) as u16;
        let port = u16::from_be(port);

        if entry.dwState == MIB_TCP_STATE_ESTAB
            && u32::from_be(entry.dwLocalAddr) == INADDR_LOOPBACK
            && u32::from_be(entry.dwRemoteAddr) == INADDR_LOOPBACK
            && port == addr.port()
        {
            return Ok(entry.dwOwningPid);
        }
    }

    Err(Error::new(ErrorKind::Other, "PID of TCP address not found"))
}

// Get the process ID of the connected peer
#[cfg(any(test, feature = "async-io"))]
pub fn tcp_stream_get_peer_pid(stream: &std::net::TcpStream) -> Result<DWORD, Error> {
    let peer_addr = stream.peer_addr()?;

    socket_addr_get_pid(&peer_addr)
}

#[cfg(any(test, feature = "async-io"))]
fn last_err() -> std::io::Error {
    use winapi::um::winsock2::WSAGetLastError;

    let err = unsafe { WSAGetLastError() };
    std::io::Error::from_raw_os_error(err)
}

// Get the process ID of the connected peer
#[cfg(feature = "async-io")]
pub fn unix_stream_get_peer_pid(stream: &UnixStream) -> Result<DWORD, Error> {
    use std::os::windows::io::AsRawSocket;
    use winapi::{
        shared::ws2def::IOC_VENDOR,
        um::winsock2::{WSAIoctl, SOCKET_ERROR},
    };

    macro_rules! _WSAIOR {
        ($x:expr, $y:expr) => {
            winapi::shared::ws2def::IOC_OUT | $x | $y
        };
    }

    let socket = stream.as_raw_socket();
    const SIO_AF_UNIX_GETPEERPID: DWORD = _WSAIOR!(IOC_VENDOR, 256);
    let mut ret = 0 as DWORD;
    let mut bytes = 0;

    let r = unsafe {
        WSAIoctl(
            socket as _,
            SIO_AF_UNIX_GETPEERPID,
            0 as *mut _,
            0,
            &mut ret as *mut _ as *mut _,
            std::mem::size_of_val(&ret) as DWORD,
            &mut bytes,
            0 as *mut _,
            None,
        )
    };

    if r == SOCKET_ERROR {
        return Err(last_err());
    }

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_pid_and_sid() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::net::TcpStream::connect(addr).unwrap();
        let _server = listener.incoming().next().unwrap().unwrap();

        let pid = tcp_stream_get_peer_pid(&client).unwrap();
        let process_token = ProcessToken::open(if pid != 0 { Some(pid) } else { None }).unwrap();
        let sid = process_token.sid().unwrap();
        assert!(!sid.is_empty());
    }
}
