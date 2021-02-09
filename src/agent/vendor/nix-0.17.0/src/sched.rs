use libc;
use {Errno, Result};

#[cfg(any(target_os = "android", target_os = "linux"))]
pub use self::sched_linux_like::*;

#[cfg(any(target_os = "android", target_os = "linux"))]
mod sched_linux_like {
    use errno::Errno;
    use libc::{self, c_int, c_void};
    use std::mem;
    use std::option::Option;
    use std::os::unix::io::RawFd;
    use unistd::Pid;
    use {Error, Result};

    // For some functions taking with a parameter of type CloneFlags,
    // only a subset of these flags have an effect.
    libc_bitflags! {
        pub struct CloneFlags: c_int {
            CLONE_VM;
            CLONE_FS;
            CLONE_FILES;
            CLONE_SIGHAND;
            CLONE_PTRACE;
            CLONE_VFORK;
            CLONE_PARENT;
            CLONE_THREAD;
            CLONE_NEWNS;
            CLONE_SYSVSEM;
            CLONE_SETTLS;
            CLONE_PARENT_SETTID;
            CLONE_CHILD_CLEARTID;
            CLONE_DETACHED;
            CLONE_UNTRACED;
            CLONE_CHILD_SETTID;
            CLONE_NEWCGROUP;
            CLONE_NEWUTS;
            CLONE_NEWIPC;
            CLONE_NEWUSER;
            CLONE_NEWPID;
            CLONE_NEWNET;
            CLONE_IO;
        }
    }

    pub type CloneCb<'a> = Box<dyn FnMut() -> isize + 'a>;

    /// CpuSet represent a bit-mask of CPUs.
    /// CpuSets are used by sched_setaffinity and
    /// sched_getaffinity for example.
    ///
    /// This is a wrapper around `libc::cpu_set_t`.
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct CpuSet {
        cpu_set: libc::cpu_set_t,
    }

    impl CpuSet {
        /// Create a new and empty CpuSet.
        pub fn new() -> CpuSet {
            CpuSet {
                cpu_set: unsafe { mem::zeroed() },
            }
        }

        /// Test to see if a CPU is in the CpuSet.
        /// `field` is the CPU id to test
        pub fn is_set(&self, field: usize) -> Result<bool> {
            if field >= CpuSet::count() {
                Err(Error::Sys(Errno::EINVAL))
            } else {
                Ok(unsafe { libc::CPU_ISSET(field, &self.cpu_set) })
            }
        }

        /// Add a CPU to CpuSet.
        /// `field` is the CPU id to add
        pub fn set(&mut self, field: usize) -> Result<()> {
            if field >= CpuSet::count() {
                Err(Error::Sys(Errno::EINVAL))
            } else {
                Ok(unsafe { libc::CPU_SET(field, &mut self.cpu_set) })
            }
        }

        /// Remove a CPU from CpuSet.
        /// `field` is the CPU id to remove
        pub fn unset(&mut self, field: usize) -> Result<()> {
            if field >= CpuSet::count() {
                Err(Error::Sys(Errno::EINVAL))
            } else {
                Ok(unsafe { libc::CPU_CLR(field, &mut self.cpu_set) })
            }
        }

        /// Return the maximum number of CPU in CpuSet
        pub fn count() -> usize {
            8 * mem::size_of::<libc::cpu_set_t>()
        }
    }

    /// `sched_setaffinity` set a thread's CPU affinity mask
    /// ([`sched_setaffinity(2)`](http://man7.org/linux/man-pages/man2/sched_setaffinity.2.html))
    ///
    /// `pid` is the thread ID to update.
    /// If pid is zero, then the calling thread is updated.
    ///
    /// The `cpuset` argument specifies the set of CPUs on which the thread
    /// will be eligible to run.
    ///
    /// # Example
    ///
    /// Binding the current thread to CPU 0 can be done as follows:
    ///
    /// ```rust,no_run
    /// use nix::sched::{CpuSet, sched_setaffinity};
    /// use nix::unistd::Pid;
    ///
    /// let mut cpu_set = CpuSet::new();
    /// cpu_set.set(0);
    /// sched_setaffinity(Pid::from_raw(0), &cpu_set);
    /// ```
    pub fn sched_setaffinity(pid: Pid, cpuset: &CpuSet) -> Result<()> {
        let res = unsafe {
            libc::sched_setaffinity(
                pid.into(),
                mem::size_of::<CpuSet>() as libc::size_t,
                &cpuset.cpu_set,
            )
        };

        Errno::result(res).map(drop)
    }

    /// `sched_getaffinity` get a thread's CPU affinity mask
    /// ([`sched_getaffinity(2)`](http://man7.org/linux/man-pages/man2/sched_getaffinity.2.html))
    ///
    /// `pid` is the thread ID to check.
    /// If pid is zero, then the calling thread is checked.
    ///
    /// Returned `cpuset` is the set of CPUs on which the thread
    /// is eligible to run.
    ///
    /// # Example
    ///
    /// Checking if the current thread can run on CPU 0 can be done as follows:
    ///
    /// ```rust,no_run
    /// use nix::sched::sched_getaffinity;
    /// use nix::unistd::Pid;
    ///
    /// let cpu_set = sched_getaffinity(Pid::from_raw(0)).unwrap();
    /// if cpu_set.is_set(0).unwrap() {
    ///     println!("Current thread can run on CPU 0");
    /// }
    /// ```
    pub fn sched_getaffinity(pid: Pid) -> Result<CpuSet> {
        let mut cpuset = CpuSet::new();
        let res = unsafe {
            libc::sched_getaffinity(
                pid.into(),
                mem::size_of::<CpuSet>() as libc::size_t,
                &mut cpuset.cpu_set,
            )
        };

        Errno::result(res).and(Ok(cpuset))
    }

    pub fn clone(
        mut cb: CloneCb,
        stack: &mut [u8],
        flags: CloneFlags,
        signal: Option<c_int>,
    ) -> Result<Pid> {
        extern "C" fn callback(data: *mut CloneCb) -> c_int {
            let cb: &mut CloneCb = unsafe { &mut *data };
            (*cb)() as c_int
        }

        let res = unsafe {
            let combined = flags.bits() | signal.unwrap_or(0);
            let ptr = stack.as_mut_ptr().offset(stack.len() as isize);
            let ptr_aligned = ptr.offset((ptr as usize % 16) as isize * -1);
            libc::clone(
                mem::transmute(
                    callback as extern "C" fn(*mut Box<dyn FnMut() -> isize>) -> i32,
                ),
                ptr_aligned as *mut c_void,
                combined,
                &mut cb as *mut _ as *mut c_void,
            )
        };

        Errno::result(res).map(Pid::from_raw)
    }

    pub fn unshare(flags: CloneFlags) -> Result<()> {
        let res = unsafe { libc::unshare(flags.bits()) };

        Errno::result(res).map(drop)
    }

    pub fn setns(fd: RawFd, nstype: CloneFlags) -> Result<()> {
        let res = unsafe { libc::setns(fd, nstype.bits()) };

        Errno::result(res).map(drop)
    }
}

/// Explicitly yield the processor to other threads.
///
/// [Further reading](http://pubs.opengroup.org/onlinepubs/9699919799/functions/sched_yield.html)
pub fn sched_yield() -> Result<()> {
    let res = unsafe { libc::sched_yield() };

    Errno::result(res).map(drop)
}
