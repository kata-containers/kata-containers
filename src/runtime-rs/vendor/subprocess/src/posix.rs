use std::env;
use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::io::{Error, Result};
use std::iter;
use std::marker::PhantomData;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::ptr;
use std::rc::Rc;
use std::time::{Duration, Instant};

use libc::{c_char, c_int};

use crate::os_common::{ExitStatus, StandardStream};

pub use libc::{ECHILD, ENOSPC};

fn check_err<T: Ord + Default>(num: T) -> Result<T> {
    if num < T::default() {
        return Err(Error::last_os_error());
    }
    Ok(num)
}

pub fn pipe() -> Result<(File, File)> {
    let mut fds = [0 as c_int; 2];
    check_err(unsafe { libc::pipe(fds.as_mut_ptr()) })?;
    Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
}

// marked unsafe because the child must not allocate before exec-ing
pub unsafe fn fork() -> Result<Option<u32>> {
    let pid = check_err(libc::fork())?;
    if pid == 0 {
        Ok(None) // child
    } else {
        Ok(Some(pid as u32)) // parent
    }
}

pub fn setuid(uid: u32) -> Result<()> {
    check_err(unsafe { libc::setuid(uid as libc::uid_t) })?;
    Ok(())
}

pub fn setgid(gid: u32) -> Result<()> {
    check_err(unsafe { libc::setgid(gid as libc::gid_t) })?;
    Ok(())
}

pub fn setpgid(pid: u32, pgid: u32) -> Result<()> {
    check_err(unsafe { libc::setpgid(pid as _, pgid as _) })?;
    Ok(())
}

fn os_to_cstring(s: &OsStr) -> Result<CString> {
    // Like CString::new, but returns an io::Result for consistency with
    // everything else.
    CString::new(s.as_bytes()).map_err(|_| Error::from_raw_os_error(libc::EINVAL))
}

#[derive(Debug)]
struct CVec {
    // Individual C strings.  Each element self.ptrs[i] points to the
    // data of self.strings[i].as_bytes_with_nul().as_ptr().
    #[allow(dead_code)]
    strings: Vec<CString>,

    // nullptr-terminated vector of pointers to data inside
    // self.strings.
    ptrs: Vec<*const c_char>,
}

impl CVec {
    fn new(slice: &[impl AsRef<OsStr>]) -> Result<CVec> {
        let maybe_strings: Result<Vec<CString>> =
            slice.iter().map(|x| os_to_cstring(x.as_ref())).collect();
        let strings = maybe_strings?;
        let ptrs: Vec<_> = strings
            .iter()
            .map(|s| s.as_bytes_with_nul().as_ptr() as _)
            .chain(iter::once(ptr::null()))
            .collect();
        Ok(CVec { strings, ptrs })
    }

    pub fn as_c_vec(&self) -> *const *const c_char {
        self.ptrs.as_ptr()
    }
}

fn split_path(mut path: &OsStr) -> impl Iterator<Item = &OsStr> {
    // Can't use `env::split`_path because it allocates OsString objects, and
    // we need to iterate over PATH after fork() when allocations are strictly
    // verboten.  We can't use `str::split()` either because PATH is an
    // `OsStr`, and there is no `OsStr::split()`.
    std::iter::from_fn(move || {
        while let Some(pos) = path.as_bytes().iter().position(|&c| c == b':') {
            let piece = OsStr::from_bytes(&path.as_bytes()[..pos]);
            path = OsStr::from_bytes(&path.as_bytes()[pos + 1..]);
            if !piece.is_empty() {
                return Some(piece);
            }
        }
        let piece = path;
        path = OsStr::new("");
        if !piece.is_empty() {
            return Some(piece);
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::split_path;
    use std;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    fn s(s: &str) -> Vec<&str> {
        split_path(OsStr::new(s))
            .map(|osstr| std::str::from_utf8(osstr.as_bytes()).unwrap())
            .collect()
    }

    #[test]
    fn test_split_path() {
        let empty = Vec::<&OsStr>::new();

        assert_eq!(s("a:b"), vec!["a", "b"]);
        assert_eq!(s("one:twothree"), vec!["one", "twothree"]);
        assert_eq!(s("a:"), vec!["a"]);
        assert_eq!(s(""), empty);
        assert_eq!(s(":"), empty);
        assert_eq!(s("::"), empty);
        assert_eq!(s(":::"), empty);
        assert_eq!(s("a::b"), vec!["a", "b"]);
        assert_eq!(s(":a::::b:"), vec!["a", "b"]);
    }
}

struct PrepExec {
    cmd: OsString,
    argvec: CVec,
    envvec: Option<CVec>,
    search_path: Option<OsString>,
    prealloc_exe: Vec<u8>,
}

impl PrepExec {
    fn new(
        cmd: OsString,
        argvec: CVec,
        envvec: Option<CVec>,
        search_path: Option<OsString>,
    ) -> PrepExec {
        // Avoid allocation after fork() by pre-allocating the buffer
        // that will be used for constructing the executable C string.

        // Allocate enough room for "<pathdir>/<command>\0", pathdir
        // being the longest component of PATH.
        let mut max_exe_len = cmd.len() + 1;
        if let Some(ref search_path) = search_path {
            // make sure enough room is present for the largest of the
            // PATH components, plus 1 for the intervening '/'.
            max_exe_len += 1 + split_path(search_path).map(OsStr::len).max().unwrap_or(0);
        }

        PrepExec {
            cmd,
            argvec,
            envvec,
            search_path,
            prealloc_exe: Vec::with_capacity(max_exe_len),
        }
    }

    fn exec(mut self) -> Result<()> {
        // Invoked after fork() - no heap allocation allowed
        let mut exe = std::mem::take(&mut self.prealloc_exe);

        if let Some(ref search_path) = self.search_path {
            let mut err = Ok(());
            // POSIX requires execvp and execve, but not execvpe (although
            // glibc provides one), so we have to iterate over PATH ourselves
            for dir in split_path(search_path.as_os_str()) {
                err = self.libc_exec(PrepExec::assemble_exe(
                    &mut exe,
                    &[dir.as_bytes(), b"/", self.cmd.as_bytes()],
                ));
                // if exec succeeds, we won't run anymore; if we're here, it failed
                assert!(err.is_err());
            }
            // we haven't found the command anywhere on the path, just return
            // the last error
            return err;
        }

        self.libc_exec(PrepExec::assemble_exe(&mut exe, &[self.cmd.as_bytes()]))?;

        // failed exec can only return Err(..)
        unreachable!();
    }

    fn assemble_exe<'a>(storage: &'a mut Vec<u8>, components: &[&[u8]]) -> &'a [u8] {
        storage.truncate(0);
        for comp in components {
            storage.extend_from_slice(comp);
        }
        // `storage` will be passed to libc::execve so it must end with \0.
        storage.push(0u8);
        storage.as_slice()
    }

    fn libc_exec(&self, exe: &[u8]) -> Result<()> {
        unsafe {
            match self.envvec.as_ref() {
                Some(envvec) => {
                    libc::execve(exe.as_ptr() as _, self.argvec.as_c_vec(), envvec.as_c_vec())
                }
                None => libc::execv(exe.as_ptr() as _, self.argvec.as_c_vec()),
            }
        };
        Err(Error::last_os_error())
    }
}

/// Prepare everything needed to `exec()` the provided `cmd` after `fork()`.
///
/// Since code executed in the child after a `fork()` is not allowed to
/// allocate (because the lock might be held), this allocates everything
/// beforehand.

pub fn prep_exec(
    cmd: impl AsRef<OsStr>,
    args: &[impl AsRef<OsStr>],
    env: Option<&[impl AsRef<OsStr>]>,
) -> Result<impl FnOnce() -> Result<()>> {
    let cmd = cmd.as_ref().to_owned();
    let argvec = CVec::new(args)?;
    let envvec = if let Some(env) = env {
        Some(CVec::new(env)?)
    } else {
        None
    };

    let search_path = if !cmd.as_bytes().iter().any(|&b| b == b'/') {
        env::var_os("PATH")
            // treat empty path as non-existent
            .and_then(|p| if p.len() == 0 { None } else { Some(p) })
    } else {
        None
    };

    // Allocate now and return a closure that just does the exec.
    let prep = PrepExec::new(cmd, argvec, envvec, search_path);
    Ok(move || prep.exec())
}

pub fn _exit(status: u8) -> ! {
    unsafe { libc::_exit(status as c_int) }
}

pub const WNOHANG: i32 = libc::WNOHANG;

pub fn waitpid(pid: u32, flags: i32) -> Result<(u32, ExitStatus)> {
    let mut status = 0 as c_int;
    let pid = check_err(unsafe {
        libc::waitpid(
            pid as libc::pid_t,
            &mut status as *mut c_int,
            flags as c_int,
        )
    })?;
    Ok((pid as u32, decode_exit_status(status)))
}

fn decode_exit_status(status: i32) -> ExitStatus {
    if libc::WIFEXITED(status) {
        ExitStatus::Exited(libc::WEXITSTATUS(status) as u32)
    } else if libc::WIFSIGNALED(status) {
        ExitStatus::Signaled(libc::WTERMSIG(status) as u8)
    } else {
        ExitStatus::Other(status)
    }
}

pub use libc::{SIGKILL, SIGTERM};

pub fn kill(pid: u32, signal: i32) -> Result<()> {
    check_err(unsafe { libc::kill(pid as c_int, signal) })?;
    Ok(())
}

pub const F_GETFD: i32 = libc::F_GETFD;
pub const F_SETFD: i32 = libc::F_SETFD;
pub const FD_CLOEXEC: i32 = libc::FD_CLOEXEC;

pub fn fcntl(fd: i32, cmd: i32, arg1: Option<i32>) -> Result<i32> {
    check_err(unsafe {
        match arg1 {
            Some(arg1) => libc::fcntl(fd, cmd, arg1),
            None => libc::fcntl(fd, cmd),
        }
    })
}

pub fn dup2(oldfd: i32, newfd: i32) -> Result<()> {
    check_err(unsafe { libc::dup2(oldfd, newfd) })?;
    Ok(())
}

pub fn make_standard_stream(which: StandardStream) -> Result<Rc<File>> {
    let stream = Rc::new(unsafe { File::from_raw_fd(which as RawFd) });
    // Leak the Rc so the object we return doesn't close the underlying file
    // descriptor.  We didn't open it, and it is shared by everything else, so
    // we are not allowed to close it either.
    mem::forget(Rc::clone(&stream));
    Ok(stream)
}

pub fn reset_sigpipe() -> Result<()> {
    // This is called after forking to reset SIGPIPE handling to the
    // defaults that Unix programs expect.  Quoting
    // std::process::Command::do_exec:
    //
    // """
    // libstd ignores SIGPIPE, and signal-handling libraries often set
    // a mask. Child processes inherit ignored signals and the signal
    // mask from their parent, but most UNIX programs do not reset
    // these things on their own, so we need to clean things up now to
    // avoid confusing the program we're about to run.
    // """

    unsafe {
        let mut set: mem::MaybeUninit<libc::sigset_t> = mem::MaybeUninit::uninit();
        check_err(libc::sigemptyset(set.as_mut_ptr()))?;
        let set = set.assume_init();
        check_err(libc::pthread_sigmask(
            libc::SIG_SETMASK,
            &set,
            ptr::null_mut(),
        ))?;
        match libc::signal(libc::SIGPIPE, libc::SIG_DFL) {
            libc::SIG_ERR => return Err(Error::last_os_error()),
            _ => (),
        }
    }
    Ok(())
}

#[repr(C)]
pub struct PollFd<'a>(libc::pollfd, PhantomData<&'a ()>);

impl PollFd<'_> {
    pub fn new<'a>(file: Option<&'a File>, events: i16) -> PollFd<'a> {
        PollFd(
            libc::pollfd {
                fd: file.map(File::as_raw_fd).unwrap_or(-1),
                events,
                revents: 0,
            },
            PhantomData,
        )
    }

    pub fn test(&self, mask: i16) -> bool {
        self.0.revents & mask != 0
    }
}

pub use libc::{POLLERR, POLLHUP, POLLIN, POLLNVAL, POLLOUT, POLLPRI};

pub fn poll(fds: &mut [PollFd<'_>], mut timeout: Option<Duration>) -> Result<usize> {
    let deadline = timeout.map(|timeout| Instant::now() + timeout);
    loop {
        // poll() accepts a maximum timeout of 2**31-1 ms, which is
        // less than 25 days.  The caller can specify Durations much
        // larger than that, so support them by waiting in a loop.
        let (timeout_ms, overflow) = timeout
            .map(|timeout| {
                let timeout = timeout.as_millis();
                if timeout <= i32::max_value() as u128 {
                    (timeout as i32, false)
                } else {
                    (i32::max_value(), true)
                }
            })
            .unwrap_or((-1, false));
        let fds_ptr = fds.as_ptr() as *mut libc::pollfd;
        let cnt = unsafe { check_err(libc::poll(fds_ptr, fds.len() as libc::nfds_t, timeout_ms))? };
        if cnt != 0 || !overflow {
            return Ok(cnt as usize);
        }

        let deadline = deadline.unwrap();
        let now = Instant::now();
        if now >= deadline {
            return Ok(0);
        }
        timeout = Some(deadline - now);
    }
}
