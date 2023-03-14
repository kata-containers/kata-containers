use std::ffi::CString;
use std::io;
use std::process::Command;
use std::ptr;
use std::sync::Arc;
use std::thread::{Builder, JoinHandle};

#[derive(Debug)]
pub struct Client {
    sem: Handle,
    name: String,
}

#[derive(Debug)]
pub struct Acquired;

type BOOL = i32;
type DWORD = u32;
type HANDLE = *mut u8;
type LONG = i32;

const ERROR_ALREADY_EXISTS: DWORD = 183;
const FALSE: BOOL = 0;
const INFINITE: DWORD = 0xffffffff;
const SEMAPHORE_MODIFY_STATE: DWORD = 0x2;
const SYNCHRONIZE: DWORD = 0x00100000;
const TRUE: BOOL = 1;
const WAIT_OBJECT_0: DWORD = 0;

extern "system" {
    fn CloseHandle(handle: HANDLE) -> BOOL;
    fn SetEvent(hEvent: HANDLE) -> BOOL;
    fn WaitForMultipleObjects(
        ncount: DWORD,
        lpHandles: *const HANDLE,
        bWaitAll: BOOL,
        dwMilliseconds: DWORD,
    ) -> DWORD;
    fn CreateEventA(
        lpEventAttributes: *mut u8,
        bManualReset: BOOL,
        bInitialState: BOOL,
        lpName: *const i8,
    ) -> HANDLE;
    fn ReleaseSemaphore(
        hSemaphore: HANDLE,
        lReleaseCount: LONG,
        lpPreviousCount: *mut LONG,
    ) -> BOOL;
    fn CreateSemaphoreA(
        lpEventAttributes: *mut u8,
        lInitialCount: LONG,
        lMaximumCount: LONG,
        lpName: *const i8,
    ) -> HANDLE;
    fn OpenSemaphoreA(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: *const i8) -> HANDLE;
    fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    #[link_name = "SystemFunction036"]
    fn RtlGenRandom(RandomBuffer: *mut u8, RandomBufferLength: u32) -> u8;
}

// Note that we ideally would use the `getrandom` crate, but unfortunately
// that causes build issues when this crate is used in rust-lang/rust (see
// rust-lang/rust#65014 for more information). As a result we just inline
// the pretty simple Windows-specific implementation of generating
// randomness.
fn getrandom(dest: &mut [u8]) -> io::Result<()> {
    // Prevent overflow of u32
    for chunk in dest.chunks_mut(u32::max_value() as usize) {
        let ret = unsafe { RtlGenRandom(chunk.as_mut_ptr(), chunk.len() as u32) };
        if ret == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to generate random bytes",
            ));
        }
    }
    Ok(())
}

impl Client {
    pub fn new(limit: usize) -> io::Result<Client> {
        // Try a bunch of random semaphore names until we get a unique one,
        // but don't try for too long.
        //
        // Note that `limit == 0` is a valid argument above but Windows
        // won't let us create a semaphore with 0 slots available to it. Get
        // `limit == 0` working by creating a semaphore instead with one
        // slot and then immediately acquire it (without ever releaseing it
        // back).
        for _ in 0..100 {
            let mut bytes = [0; 4];
            getrandom(&mut bytes)?;
            let mut name = format!("__rust_jobserver_semaphore_{}\0", u32::from_ne_bytes(bytes));
            unsafe {
                let create_limit = if limit == 0 { 1 } else { limit };
                let r = CreateSemaphoreA(
                    ptr::null_mut(),
                    create_limit as LONG,
                    create_limit as LONG,
                    name.as_ptr() as *const _,
                );
                if r.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let handle = Handle(r);

                let err = io::Error::last_os_error();
                if err.raw_os_error() == Some(ERROR_ALREADY_EXISTS as i32) {
                    continue;
                }
                name.pop(); // chop off the trailing nul
                let client = Client {
                    sem: handle,
                    name: name,
                };
                if create_limit != limit {
                    client.acquire()?;
                }
                return Ok(client);
            }
        }

        Err(io::Error::new(
            io::ErrorKind::Other,
            "failed to find a unique name for a semaphore",
        ))
    }

    pub unsafe fn open(s: &str) -> Option<Client> {
        let name = match CString::new(s) {
            Ok(s) => s,
            Err(_) => return None,
        };

        let sem = OpenSemaphoreA(SYNCHRONIZE | SEMAPHORE_MODIFY_STATE, FALSE, name.as_ptr());
        if sem.is_null() {
            None
        } else {
            Some(Client {
                sem: Handle(sem),
                name: s.to_string(),
            })
        }
    }

    pub fn acquire(&self) -> io::Result<Acquired> {
        unsafe {
            let r = WaitForSingleObject(self.sem.0, INFINITE);
            if r == WAIT_OBJECT_0 {
                Ok(Acquired)
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }

    pub fn release(&self, _data: Option<&Acquired>) -> io::Result<()> {
        unsafe {
            let r = ReleaseSemaphore(self.sem.0, 1, ptr::null_mut());
            if r != 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }

    pub fn string_arg(&self) -> String {
        self.name.clone()
    }

    pub fn configure(&self, _cmd: &mut Command) {
        // nothing to do here, we gave the name of our semaphore to the
        // child above
    }
}

#[derive(Debug)]
struct Handle(HANDLE);
// HANDLE is a raw ptr, but we're send/sync
unsafe impl Sync for Handle {}
unsafe impl Send for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[derive(Debug)]
pub struct Helper {
    event: Arc<Handle>,
    thread: JoinHandle<()>,
}

pub(crate) fn spawn_helper(
    client: crate::Client,
    state: Arc<super::HelperState>,
    mut f: Box<dyn FnMut(io::Result<crate::Acquired>) + Send>,
) -> io::Result<Helper> {
    let event = unsafe {
        let r = CreateEventA(ptr::null_mut(), TRUE, FALSE, ptr::null());
        if r.is_null() {
            return Err(io::Error::last_os_error());
        } else {
            Handle(r)
        }
    };
    let event = Arc::new(event);
    let event2 = event.clone();
    let thread = Builder::new().spawn(move || {
        let objects = [event2.0, client.inner.sem.0];
        state.for_each_request(|_| {
            const WAIT_OBJECT_1: u32 = WAIT_OBJECT_0 + 1;
            match unsafe { WaitForMultipleObjects(2, objects.as_ptr(), FALSE, INFINITE) } {
                WAIT_OBJECT_0 => return,
                WAIT_OBJECT_1 => f(Ok(crate::Acquired {
                    client: client.inner.clone(),
                    data: Acquired,
                    disabled: false,
                })),
                _ => f(Err(io::Error::last_os_error())),
            }
        });
    })?;
    Ok(Helper { thread, event })
}

impl Helper {
    pub fn join(self) {
        // Unlike unix this logic is much easier. If our thread was blocked
        // in waiting for requests it should already be woken up and
        // exiting. Otherwise it's waiting for a token, so we wake it up
        // with a different event that it's also waiting on here. After
        // these two we should be guaranteed the thread is on its way out,
        // so we can safely `join`.
        let r = unsafe { SetEvent(self.event.0) };
        if r == 0 {
            panic!("failed to set event: {}", io::Error::last_os_error());
        }
        drop(self.thread.join());
    }
}
