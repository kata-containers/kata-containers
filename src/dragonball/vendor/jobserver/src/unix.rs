use libc::c_int;

use std::fs::File;
use std::io::{self, Read, Write};
use std::mem;
use std::mem::MaybeUninit;
use std::os::unix::prelude::*;
use std::process::Command;
use std::ptr;
use std::sync::{Arc, Once};
use std::thread::{self, Builder, JoinHandle};
use std::time::Duration;

#[derive(Debug)]
pub struct Client {
    read: File,
    write: File,
}

#[derive(Debug)]
pub struct Acquired {
    byte: u8,
}

impl Client {
    pub fn new(mut limit: usize) -> io::Result<Client> {
        let client = unsafe { Client::mk()? };

        // I don't think the character written here matters, but I could be
        // wrong!
        const BUFFER: [u8; 128] = [b'|'; 128];

        set_nonblocking(client.write.as_raw_fd(), true)?;

        while limit > 0 {
            let n = limit.min(BUFFER.len());

            (&client.write).write_all(&BUFFER[..n])?;
            limit -= n;
        }

        set_nonblocking(client.write.as_raw_fd(), false)?;

        Ok(client)
    }

    unsafe fn mk() -> io::Result<Client> {
        let mut pipes = [0; 2];

        // Attempt atomically-create-with-cloexec if we can on Linux,
        // detected by using the `syscall` function in `libc` to try to work
        // with as many kernels/glibc implementations as possible.
        #[cfg(target_os = "linux")]
        {
            use std::sync::atomic::{AtomicBool, Ordering};

            static PIPE2_AVAILABLE: AtomicBool = AtomicBool::new(true);
            if PIPE2_AVAILABLE.load(Ordering::SeqCst) {
                match libc::syscall(libc::SYS_pipe2, pipes.as_mut_ptr(), libc::O_CLOEXEC) {
                    -1 => {
                        let err = io::Error::last_os_error();
                        if err.raw_os_error() == Some(libc::ENOSYS) {
                            PIPE2_AVAILABLE.store(false, Ordering::SeqCst);
                        } else {
                            return Err(err);
                        }
                    }
                    _ => return Ok(Client::from_fds(pipes[0], pipes[1])),
                }
            }
        }

        cvt(libc::pipe(pipes.as_mut_ptr()))?;
        drop(set_cloexec(pipes[0], true));
        drop(set_cloexec(pipes[1], true));
        Ok(Client::from_fds(pipes[0], pipes[1]))
    }

    pub unsafe fn open(s: &str) -> Option<Client> {
        let mut parts = s.splitn(2, ',');
        let read = parts.next().unwrap();
        let write = match parts.next() {
            Some(s) => s,
            None => return None,
        };

        let read = match read.parse() {
            Ok(n) => n,
            Err(_) => return None,
        };
        let write = match write.parse() {
            Ok(n) => n,
            Err(_) => return None,
        };

        // Ok so we've got two integers that look like file descriptors, but
        // for extra sanity checking let's see if they actually look like
        // instances of a pipe before we return the client.
        //
        // If we're called from `make` *without* the leading + on our rule
        // then we'll have `MAKEFLAGS` env vars but won't actually have
        // access to the file descriptors.
        if is_valid_fd(read) && is_valid_fd(write) {
            drop(set_cloexec(read, true));
            drop(set_cloexec(write, true));
            Some(Client::from_fds(read, write))
        } else {
            None
        }
    }

    unsafe fn from_fds(read: c_int, write: c_int) -> Client {
        Client {
            read: File::from_raw_fd(read),
            write: File::from_raw_fd(write),
        }
    }

    pub fn acquire(&self) -> io::Result<Acquired> {
        // Ignore interrupts and keep trying if that happens
        loop {
            if let Some(token) = self.acquire_allow_interrupts()? {
                return Ok(token);
            }
        }
    }

    /// Block waiting for a token, returning `None` if we're interrupted with
    /// EINTR.
    fn acquire_allow_interrupts(&self) -> io::Result<Option<Acquired>> {
        // We don't actually know if the file descriptor here is set in
        // blocking or nonblocking mode. AFAIK all released versions of
        // `make` use blocking fds for the jobserver, but the unreleased
        // version of `make` doesn't. In the unreleased version jobserver
        // fds are set to nonblocking and combined with `pselect`
        // internally.
        //
        // Here we try to be compatible with both strategies. We optimistically
        // try to read from the file descriptor which then may block, return
        // a token or indicate that polling is needed.
        // Blocking reads (if possible) allows the kernel to be more selective
        // about which readers to wake up when a token is written to the pipe.
        //
        // We use `poll` here to block this thread waiting for read
        // readiness, and then afterwards we perform the `read` itself. If
        // the `read` returns that it would block then we start over and try
        // again.
        //
        // Also note that we explicitly don't handle EINTR here. That's used
        // to shut us down, so we otherwise punt all errors upwards.
        unsafe {
            let mut fd: libc::pollfd = mem::zeroed();
            fd.fd = self.read.as_raw_fd();
            fd.events = libc::POLLIN;
            loop {
                let mut buf = [0];
                match (&self.read).read(&mut buf) {
                    Ok(1) => return Ok(Some(Acquired { byte: buf[0] })),
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            "early EOF on jobserver pipe",
                        ))
                    }
                    Err(e) => match e.kind() {
                        io::ErrorKind::WouldBlock => { /* fall through to polling */ }
                        io::ErrorKind::Interrupted => return Ok(None),
                        _ => return Err(e),
                    },
                }

                loop {
                    fd.revents = 0;
                    if libc::poll(&mut fd, 1, -1) == -1 {
                        let e = io::Error::last_os_error();
                        return match e.kind() {
                            io::ErrorKind::Interrupted => Ok(None),
                            _ => Err(e),
                        };
                    }
                    if fd.revents != 0 {
                        break;
                    }
                }
            }
        }
    }

    pub fn release(&self, data: Option<&Acquired>) -> io::Result<()> {
        // Note that the fd may be nonblocking but we're going to go ahead
        // and assume that the writes here are always nonblocking (we can
        // always quickly release a token). If that turns out to not be the
        // case we'll get an error anyway!
        let byte = data.map(|d| d.byte).unwrap_or(b'+');
        match (&self.write).write(&[byte])? {
            1 => Ok(()),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to write token back to jobserver",
            )),
        }
    }

    pub fn string_arg(&self) -> String {
        format!("{},{}", self.read.as_raw_fd(), self.write.as_raw_fd())
    }

    pub fn available(&self) -> io::Result<usize> {
        let mut len = MaybeUninit::<c_int>::uninit();
        cvt(unsafe { libc::ioctl(self.read.as_raw_fd(), libc::FIONREAD, len.as_mut_ptr()) })?;
        Ok(unsafe { len.assume_init() } as usize)
    }

    pub fn configure(&self, cmd: &mut Command) {
        // Here we basically just want to say that in the child process
        // we'll configure the read/write file descriptors to *not* be
        // cloexec, so they're inherited across the exec and specified as
        // integers through `string_arg` above.
        let read = self.read.as_raw_fd();
        let write = self.write.as_raw_fd();
        unsafe {
            cmd.pre_exec(move || {
                set_cloexec(read, false)?;
                set_cloexec(write, false)?;
                Ok(())
            });
        }
    }
}

#[derive(Debug)]
pub struct Helper {
    thread: JoinHandle<()>,
    state: Arc<super::HelperState>,
}

pub(crate) fn spawn_helper(
    client: crate::Client,
    state: Arc<super::HelperState>,
    mut f: Box<dyn FnMut(io::Result<crate::Acquired>) + Send>,
) -> io::Result<Helper> {
    static USR1_INIT: Once = Once::new();
    let mut err = None;
    USR1_INIT.call_once(|| unsafe {
        let mut new: libc::sigaction = mem::zeroed();
        new.sa_sigaction = sigusr1_handler as usize;
        new.sa_flags = libc::SA_SIGINFO as _;
        if libc::sigaction(libc::SIGUSR1, &new, ptr::null_mut()) != 0 {
            err = Some(io::Error::last_os_error());
        }
    });

    if let Some(e) = err.take() {
        return Err(e);
    }

    let state2 = state.clone();
    let thread = Builder::new().spawn(move || {
        state2.for_each_request(|helper| loop {
            match client.inner.acquire_allow_interrupts() {
                Ok(Some(data)) => {
                    break f(Ok(crate::Acquired {
                        client: client.inner.clone(),
                        data,
                        disabled: false,
                    }))
                }
                Err(e) => break f(Err(e)),
                Ok(None) if helper.producer_done() => break,
                Ok(None) => {}
            }
        });
    })?;

    Ok(Helper { thread, state })
}

impl Helper {
    pub fn join(self) {
        let dur = Duration::from_millis(10);
        let mut state = self.state.lock();
        debug_assert!(state.producer_done);

        // We need to join our helper thread, and it could be blocked in one
        // of two locations. First is the wait for a request, but the
        // initial drop of `HelperState` will take care of that. Otherwise
        // it may be blocked in `client.acquire()`. We actually have no way
        // of interrupting that, so resort to `pthread_kill` as a fallback.
        // This signal should interrupt any blocking `read` call with
        // `io::ErrorKind::Interrupt` and cause the thread to cleanly exit.
        //
        // Note that we don't do this forever though since there's a chance
        // of bugs, so only do this opportunistically to make a best effort
        // at clearing ourselves up.
        for _ in 0..100 {
            if state.consumer_done {
                break;
            }
            unsafe {
                // Ignore the return value here of `pthread_kill`,
                // apparently on OSX if you kill a dead thread it will
                // return an error, but on other platforms it may not. In
                // that sense we don't actually know if this will succeed or
                // not!
                libc::pthread_kill(self.thread.as_pthread_t() as _, libc::SIGUSR1);
            }
            state = self
                .state
                .cvar
                .wait_timeout(state, dur)
                .unwrap_or_else(|e| e.into_inner())
                .0;
            thread::yield_now(); // we really want the other thread to run
        }

        // If we managed to actually see the consumer get done, then we can
        // definitely wait for the thread. Otherwise it's... off in the ether
        // I guess?
        if state.consumer_done {
            drop(self.thread.join());
        }
    }
}

fn is_valid_fd(fd: c_int) -> bool {
    unsafe { libc::fcntl(fd, libc::F_GETFD) != -1 }
}

fn set_cloexec(fd: c_int, set: bool) -> io::Result<()> {
    unsafe {
        let previous = cvt(libc::fcntl(fd, libc::F_GETFD))?;
        let new = if set {
            previous | libc::FD_CLOEXEC
        } else {
            previous & !libc::FD_CLOEXEC
        };
        if new != previous {
            cvt(libc::fcntl(fd, libc::F_SETFD, new))?;
        }
        Ok(())
    }
}

fn set_nonblocking(fd: c_int, set: bool) -> io::Result<()> {
    let status_flag = if set { libc::O_NONBLOCK } else { 0 };

    unsafe {
        cvt(libc::fcntl(fd, libc::F_SETFL, status_flag))?;
    }

    Ok(())
}

fn cvt(t: c_int) -> io::Result<c_int> {
    if t == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(t)
    }
}

extern "C" fn sigusr1_handler(
    _signum: c_int,
    _info: *mut libc::siginfo_t,
    _ptr: *mut libc::c_void,
) {
    // nothing to do
}
