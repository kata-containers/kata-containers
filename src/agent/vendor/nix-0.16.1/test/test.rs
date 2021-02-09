extern crate bytes;
#[cfg(any(target_os = "android", target_os = "linux"))]
extern crate caps;
#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate nix;
#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate rand;
#[cfg(target_os = "freebsd")]
extern crate sysctl;
extern crate tempfile;

cfg_if! {
    if #[cfg(any(target_os = "android", target_os = "linux"))] {
        macro_rules! require_capability {
            ($capname:ident) => {
                use ::caps::{Capability, CapSet, has_cap};
                use ::std::io::{self, Write};

                if !has_cap(None, CapSet::Effective, Capability::$capname)
                    .unwrap()
                {
                    let stderr = io::stderr();
                    let mut handle = stderr.lock();
                    writeln!(handle,
                        "Insufficient capabilities. Skipping test.")
                        .unwrap();
                    return;
                }
            }
        }
    } else {
        macro_rules! require_capability {
            ($capname:ident) => {}
        }
    }
}

#[cfg(target_os = "freebsd")]
macro_rules! skip_if_jailed {
    ($name:expr) => {
        use ::sysctl::CtlValue;

        if let CtlValue::Int(1) = ::sysctl::value("security.jail.jailed")
            .unwrap()
        {
            use ::std::io::Write;
            let stderr = ::std::io::stderr();
            let mut handle = stderr.lock();
            writeln!(handle, "{} cannot run in a jail. Skipping test.", $name)
                .unwrap();
            return;
        }
    }
}

macro_rules! skip_if_not_root {
    ($name:expr) => {
        use nix::unistd::Uid;

        if !Uid::current().is_root() {
            use ::std::io::Write;
            let stderr = ::std::io::stderr();
            let mut handle = stderr.lock();
            writeln!(handle, "{} requires root privileges. Skipping test.", $name).unwrap();
            return;
        }
    };
}

cfg_if! {
    if #[cfg(any(target_os = "android", target_os = "linux"))] {
        macro_rules! skip_if_seccomp {
            ($name:expr) => {
                if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
                    for l in s.lines() {
                        let mut fields = l.split_whitespace();
                        if fields.next() == Some("Seccomp:") &&
                            fields.next() != Some("0")
                        {
                            use ::std::io::Write;
                            let stderr = ::std::io::stderr();
                            let mut handle = stderr.lock();
                            writeln!(handle,
                                "{} cannot be run in Seccomp mode.  Skipping test.",
                                stringify!($name)).unwrap();
                            return;
                        }
                    }
                }
            }
        }
    } else {
        macro_rules! skip_if_seccomp {
            ($name:expr) => {}
        }
    }
}

mod sys;
mod test_dir;
mod test_fcntl;
#[cfg(any(target_os = "android",
          target_os = "linux"))]
mod test_kmod;
#[cfg(any(target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "fushsia",
          target_os = "linux",
          target_os = "netbsd"))]
mod test_mq;
mod test_net;
mod test_nix_path;
mod test_poll;
mod test_pty;
#[cfg(any(target_os = "android",
          target_os = "linux"))]
mod test_sched;
#[cfg(any(target_os = "android",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "linux",
          target_os = "macos"))]
mod test_sendfile;
mod test_stat;
mod test_unistd;

use std::os::unix::io::RawFd;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock, RwLockWriteGuard};
use nix::unistd::{chdir, getcwd, read};

/// Helper function analogous to `std::io::Read::read_exact`, but for `RawFD`s
fn read_exact(f: RawFd, buf: &mut  [u8]) {
    let mut len = 0;
    while len < buf.len() {
        // get_mut would be better than split_at_mut, but it requires nightly
        let (_, remaining) = buf.split_at_mut(len);
        len += read(f, remaining).unwrap();
    }
}

lazy_static! {
    /// Any test that changes the process's current working directory must grab
    /// the RwLock exclusively.  Any process that cares about the current
    /// working directory must grab it shared.
    pub static ref CWD_LOCK: RwLock<()> = RwLock::new(());
    /// Any test that creates child processes must grab this mutex, regardless
    /// of what it does with those children.
    pub static ref FORK_MTX: Mutex<()> = Mutex::new(());
    /// Any test that changes the process's supplementary groups must grab this
    /// mutex
    pub static ref GROUPS_MTX: Mutex<()> = Mutex::new(());
    /// Any tests that loads or unloads kernel modules must grab this mutex
    pub static ref KMOD_MTX: Mutex<()> = Mutex::new(());
    /// Any test that calls ptsname(3) must grab this mutex.
    pub static ref PTSNAME_MTX: Mutex<()> = Mutex::new(());
    /// Any test that alters signal handling must grab this mutex.
    pub static ref SIGNAL_MTX: Mutex<()> = Mutex::new(());
}

/// RAII object that restores a test's original directory on drop
struct DirRestore<'a> {
    d: PathBuf,
    _g: RwLockWriteGuard<'a, ()>
}

impl<'a> DirRestore<'a> {
    fn new() -> Self {
        let guard = ::CWD_LOCK.write()
            .expect("Lock got poisoned by another test");
        DirRestore{
            _g: guard,
            d: getcwd().unwrap(),
        }
    }
}

impl<'a> Drop for DirRestore<'a> {
    fn drop(&mut self) {
        let r = chdir(&self.d);
        if std::thread::panicking() {
            r.unwrap();
        }
    }
}
