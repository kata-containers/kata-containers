//! Interfaces to `prctl()` commands that don't deal with capabilities.

/// Set the name of the current thread.
///
/// If the given name is longer than 15 bytes, it will be truncated to the first 15 bytes.
///
/// (Note: Other documentation regarding Linux capabilities says that the maximum length is 16
/// bytes; that value includes the terminating NUL byte at the end of C strings.)
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[cfg(feature = "std")]
#[inline]
pub fn set_name<N: AsRef<std::ffi::OsStr>>(name: N) -> crate::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    raw_set_name(name.as_ref().as_bytes())
}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[cfg(feature = "std")]
fn raw_set_name(name: &[u8]) -> crate::Result<()> {
    if name.contains(&0) {
        return Err(crate::Error::from_code(libc::EINVAL));
    }

    let mut buf = [0; 16];
    let ptr = if name.len() < buf.len() {
        buf[..name.len()].copy_from_slice(name);
        buf.as_ptr()
    } else {
        // The kernel only looks at the first 16 bytes, so we can use the original string
        name.as_ptr()
    };

    unsafe {
        crate::raw_prctl(libc::PR_SET_NAME, ptr as libc::c_ulong, 0, 0, 0)?;
    }

    Ok(())
}

/// Get the name of the current thread.
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[cfg(feature = "std")]
pub fn get_name() -> crate::Result<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt;

    let mut name_vec = vec![0; 16];
    unsafe {
        crate::raw_prctl(
            libc::PR_GET_NAME,
            name_vec.as_ptr() as libc::c_ulong,
            0,
            0,
            0,
        )?;
    }

    name_vec.truncate(name_vec.iter().position(|x| *x == 0).unwrap());

    Ok(std::ffi::OsString::from_vec(name_vec))
}

/// Get the no-new-privileges flag of the current thread.
///
/// See [`set_no_new_privs()`](./fn.set_no_new_privs.html) for more details.
#[inline]
pub fn get_no_new_privs() -> crate::Result<bool> {
    let res = unsafe { crate::raw_prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) }?;

    Ok(res != 0)
}

/// Enable the no-new-privileges flag on the current thread.
///
/// If this flag is enabled, `execve()` will no longer honor set-user-ID/set-group-ID bits and file
/// capabilities on executables. See prctl(2) for more details.
///
/// Once this is enabled, it cannot be unset.
#[inline]
pub fn set_no_new_privs() -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)?;
    }

    Ok(())
}

/// Get the "keep capabilities" flag of the current thread.
///
/// See [`set_keepcaps()`](./fn.set_keepcaps.html) for more details.
#[inline]
pub fn get_keepcaps() -> crate::Result<bool> {
    let res = unsafe { crate::raw_prctl(libc::PR_GET_KEEPCAPS, 0, 0, 0, 0) }?;

    Ok(res != 0)
}

/// Set the "keep capabilities" flag of the current thread.
///
/// Setting this flag allows a thread to retain its permitted capabilities when switching all its
/// UIDs to non-zero values (the effective capability set is still emptied).
///
/// This flag is always cleared on an `execve()`; see capabilities(7) for more details.
#[inline]
pub fn set_keepcaps(keep: bool) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(libc::PR_SET_KEEPCAPS, keep as libc::c_ulong, 0, 0, 0)?;
    }

    Ok(())
}

/// Get the "dumpable" flag for the current process.
///
/// See [`set_dumpable()`](./fn.set_dumpable.html) for more details.
#[inline]
pub fn get_dumpable() -> crate::Result<bool> {
    let res = unsafe { crate::raw_prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) }?;

    Ok(res != 0)
}

/// Set the "dumpable" flag for the current process.
///
/// This controls whether a core dump will be produced for the process if it receives a signal that
/// would make it perform a core dump. It also restricts which processes can be attached with
/// `ptrace()`.
#[inline]
pub fn set_dumpable(dumpable: bool) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(libc::PR_SET_DUMPABLE, dumpable as libc::c_ulong, 0, 0, 0)?;
    }

    Ok(())
}

/// Set the "child subreaper" flag for the current process.
///
/// If a process dies, its children will be reparented to the nearest surviving ancestor subreaper,
/// or to PID 1 if it has no ancestor subreapers.
///
/// This is useful for process managers that need to be informed when any of their descendants
/// (possibly processes that used the double-`fork()` trick to become daemons) die.
#[inline]
pub fn set_subreaper(flag: bool) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(libc::PR_SET_CHILD_SUBREAPER, flag as libc::c_ulong, 0, 0, 0)?;
    }

    Ok(())
}

/// Get the "child subreaper" flag for the current process.
///
/// See [`set_subreaper()`](./fn.set_subreaper.html) for more detailss.
#[inline]
pub fn get_subreaper() -> crate::Result<bool> {
    let mut res = 0;

    unsafe {
        crate::raw_prctl(
            libc::PR_GET_CHILD_SUBREAPER,
            (&mut res) as *mut libc::c_int as libc::c_ulong,
            0,
            0,
            0,
        )?;
    }

    Ok(res != 0)
}

/// Set the parent-death signal of the current process.
///
/// The parent-death signal is the signal that this process will receive when its parent dies. It
/// is cleared when executing a binary that is set-UID, set-GID, or has file capabilities.
///
/// Specifying `None` is equivalent to specifying `Some(0)`; both clear the parent-death signal.
#[inline]
pub fn set_pdeathsig(sig: Option<libc::c_int>) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(
            libc::PR_SET_PDEATHSIG,
            sig.unwrap_or(0) as libc::c_ulong,
            0,
            0,
            0,
        )?;
    }

    Ok(())
}

/// Get the parent-death signal of the current process.
///
/// This returns `Ok(None)` if the process's parent-death signal is cleared, and `Ok(Some(sig))`
/// otherwise.
#[inline]
pub fn get_pdeathsig() -> crate::Result<Option<libc::c_int>> {
    let mut sig = 0;

    unsafe {
        crate::raw_prctl(
            libc::PR_GET_PDEATHSIG,
            (&mut sig) as *mut libc::c_int as libc::c_ulong,
            0,
            0,
            0,
        )?;
    }

    Ok(if sig == 0 { None } else { Some(sig) })
}

bitflags::bitflags! {
    /// Represents the thread's securebits flags.
    pub struct Secbits: libc::c_ulong {
        /// If this flag is set, the kernel does not grant capabilities when a SUID-root program is
        /// executed, or when a process with an effective/real UID of 0 calls `exec()`.
        const NOROOT = 0x1;

        /// Locks the `NOROOT` flag so it cannot be changed.
        const NOROOT_LOCKED = 0x2;

        /// If this flag is set, the kernel will not adjust the current thread's
        /// permitted/effective/inheritable capability sets when its effective and filesystem UIDs
        /// are changed between zero and nonzero values.
        ///
        const NO_SETUID_FIXUP = 0x4;
        /// Locks the `NO_SETUID_FIXUP` flag so it cannot be changed.
        const NO_SETUID_FIXUP_LOCKED = 0x8;

        /// If this flag is set, the kernel will not empty the current thread's permitted
        /// capability set when all of its UIDs are switched to nonzero values. (However, the
        /// effective capability set will still be cleared.)
        ///
        /// This flag is cleared across `execve()` calls.
        ///
        /// Note: [`get_keepcaps()`] and [`set_keepcaps()`] provide the same functionality as this
        /// flag (setting the flag via one method will change its value as perceived by the other,
        /// and vice versa). However, [`set_keepcaps()`] does not require CAP_SETPCAP; changing the
        /// securebits does. As a result, if you only need to manipulate the `KEEP_CAPS` flag, you
        /// may wish to instead use [`get_keepcaps()`] and [`set_keepcaps()`].
        ///
        /// [`get_keepcaps()`]: ./fn.get_keepcaps.html
        /// [`set_keepcaps()`]: ./fn.set_keepcaps.html
        const KEEP_CAPS = 0x10;

        /// Locks the `KEEP_CAPS` flag so it cannot be changed.
        ///
        /// Note: The `KEEP_CAPS` flag is always cleared across `execve()`, even if it is "locked"
        /// using this flag. As a result, this flag is mainly useful for locking the `KEEP_CAPS`
        /// flag in the "off" setting.
        const KEEP_CAPS_LOCKED = 0x20;

        /// Disallows raising ambient capabilities.
        const NO_CAP_AMBIENT_RAISE = 0x40;

        /// Locks the `NO_CAP_AMBIENT_RAISE_LOCKED` flag so it cannot be changed.
        const NO_CAP_AMBIENT_RAISE_LOCKED = 0x80;
    }
}

/// Get the "securebits" flags of the current thread.
///
/// See [`set_securebits()`](./fn.set_securebits.html) for more details.
#[inline]
pub fn get_securebits() -> crate::Result<Secbits> {
    let f = unsafe { crate::raw_prctl(libc::PR_GET_SECUREBITS, 0, 0, 0, 0) }?;

    Ok(Secbits::from_bits_truncate(f as libc::c_ulong))
}

/// Set the "securebits" flags of the current thread.
///
/// The secure bits control various aspects of the handling of capabilities for UID 0. See
/// [`Secbits`](struct.Secbits.html) and capabilities(7) for more details.
///
/// Note: Modifying the securebits with this function requires the CAP_SETPCAP capability.
#[inline]
pub fn set_securebits(flags: Secbits) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(libc::PR_SET_SECUREBITS, flags.bits(), 0, 0, 0)?;
    }

    Ok(())
}

/// Get the secure computing mode of the current thread.
///
/// If the thread is not in secure computing mode, this function returns `false`; if it is in
/// seccomp filter mode (and the `prctl()` syscall with the given arguments is allowed by the
/// filters) then this function returns `true`; if it is in strict computing mode then it will be
/// sent a SIGKILL signal.
#[inline]
pub fn get_seccomp() -> crate::Result<bool> {
    let res = unsafe { crate::raw_prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) }?;

    Ok(res != 0)
}

/// Enable strict secure computing mode.
///
/// After this call, any syscalls except `read()`, `write()`, `_exit()`, and `sigreturn()` will
/// cause the thread to be terminated with SIGKILL.
#[inline]
pub fn set_seccomp_strict() -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_STRICT as libc::c_ulong,
            0,
            0,
            0,
        )?;
    }

    Ok(())
}

/// Get the current timer slack value.
///
/// See [`set_timerslack()`](./fn.set_timerslack.html) for more details.
///
/// # Behavior at extreme values
///
/// This function may not work correctly (specifically, it may return strange `Err` values) if the
/// current timer slack value is larger than `libc::c_ulong::MAX - 4095` or so. Unfortunately, this
/// isn't really possible to fix because of the design of the underlying `prctl()` call. However,
/// most users are unlikely to encounter this error because timer slack values in this range are
/// generally not useful.
///
/// If you *really* need to handle values in this range, try
/// `std::fs::read_to_string("/proc/self/timerslack_ns")?.trim().parse::<libc::c_ulong>().unwrap()`
/// (only works on Linux 4.6+).
#[allow(clippy::needless_return)]
#[inline]
pub fn get_timerslack() -> crate::Result<libc::c_ulong> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "sc")] {
            return crate::sc_res_decode(unsafe {
                sc::syscall!(PRCTL, libc::PR_GET_TIMERSLACK, 0, 0, 0)
            }).map(|res| res as libc::c_ulong);
        } else {
            let res = unsafe { libc::syscall(libc::SYS_prctl, libc::PR_GET_TIMERSLACK, 0, 0, 0) };

            return if res == -1 {
                Err(crate::Error::last())
            } else {
                Ok(res as libc::c_ulong)
            };
        }
    }
}

/// Set the current timer slack value.
///
/// The timer slack value is used by the kernel to group timer expirations (`select()`,
/// `epoll_wait()`, `nanosleep()`, etc.) for the calling thread. See prctl(2) for more details.
///
/// Note: Passing a value of 0 will reset the current timer slack value to the "default" timer
/// slack value (which is inherited from the parent). Again, prctl(2) contains more information.
#[inline]
pub fn set_timerslack(new_slack: libc::c_ulong) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(libc::PR_SET_TIMERSLACK, new_slack, 0, 0, 0)?;
    }

    Ok(())
}

/// Set the status of the "THP" disable flag.
///
/// This flag provides an easy way to disable transparent huge pages process-wide. See `prctl(2)`
/// for more details.
#[inline]
pub fn set_thp_disable(disable: bool) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(libc::PR_SET_THP_DISABLE, disable as _, 0, 0, 0)?;
    }

    Ok(())
}

/// Get the status of the "THP" disable flag.
///
/// See [`set_thp_disable()`] and `prctl(2)`.
#[inline]
pub fn get_thp_disable() -> crate::Result<bool> {
    let res = unsafe { crate::raw_prctl(libc::PR_GET_THP_DISABLE, 0, 0, 0, 0) }?;

    Ok(res != 0)
}

/// A value that can be passed to [`set_ptracer()`].
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Ptracer {
    /// Clear the current process's "ptracer process ID".
    None,
    /// Disable the Yama LSM's `ptrace()` restrictions for the current process.
    Any,
    /// Allow the specified process to `ptrace()` the current process (in addition to the current
    /// process's direct ancestors).
    Pid(libc::pid_t),
}

/// Set the calling process's "ptracer process ID".
///
/// Normally, if the Yama LSM is enabled and in "restricted ptrace" mode (i.e.
/// `/proc/sys/kernel/yama/ptrace_scope` contains the value `1`), a process can only be `ptrace()`d
/// by one of its direct ancestors. This function allows a process to indicate that another process
/// can also `ptrace()` it.
///
/// For more information, see [`Ptracer`] and `prctl(2)`.
#[inline]
pub fn set_ptracer(ptracer: Ptracer) -> crate::Result<()> {
    let pid = match ptracer {
        Ptracer::None => 0,
        Ptracer::Any => crate::sys::PR_SET_PTRACER_ANY,
        Ptracer::Pid(pid) if pid <= 0 => return Err(crate::Error::from_code(libc::EINVAL)),
        Ptracer::Pid(pid) => pid as libc::c_ulong,
    };

    unsafe {
        crate::raw_prctl(libc::PR_SET_PTRACER, pid, 0, 0, 0)?;
    }
    Ok(())
}

/// The possible memory corruption kill policies.
///
/// See [`get_mce_kill()`] and [`set_mce_kill()`] for usage.
///
/// For more detailed information, see `prctl(2)`.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(i32)]
pub enum MceKill {
    /// When irrecoverable corruption is detected, immediately kill this thread if it has the
    /// corrupted page mapped.
    Early = libc::PR_MCE_KILL_EARLY,
    /// When irrecoverable corruption is detected, kill this thread if it tries to access the
    /// corrupted page.
    Late = libc::PR_MCE_KILL_LATE,
    /// Follow the system-wide default action specified in `/proc/sys/vm/memory_failure_early_kill`
    /// if irrecoverable corruption is detected.
    Default = libc::PR_MCE_KILL_DEFAULT,
}

/// Set the current thread's memory corruption kill policy.
///
/// See [`MceKill`] for the different policies that can be used.
///
/// This policy is inherited by children.
#[inline]
pub fn set_mce_kill(policy: MceKill) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(
            libc::PR_MCE_KILL,
            libc::PR_MCE_KILL_SET as _,
            policy as _,
            0,
            0,
        )?;
    }

    Ok(())
}

/// Get the current thread's memory corruption kill policy.
///
/// See [`set_mce_kill()`] for more information.
#[inline]
pub fn get_mce_kill() -> crate::Result<MceKill> {
    let res = unsafe { crate::raw_prctl(libc::PR_MCE_KILL_GET, 0, 0, 0, 0) }?;

    Ok(match res {
        libc::PR_MCE_KILL_EARLY => MceKill::Early,
        libc::PR_MCE_KILL_LATE => MceKill::Late,
        libc::PR_MCE_KILL_DEFAULT => MceKill::Default,
        _ => unreachable!(),
    })
}

/// Get this thread's `clear_child_tid` address.
///
/// See `prctl(2)`, `set_tid_address(2)`, and `clone(2)` for more information.
///
/// Note: This function accounts for the 32-bit issues on x86 and MIPS that `prctl(2)` warns about.
#[inline]
pub fn get_tid_address() -> crate::Result<*mut libc::c_int> {
    cfg_if::cfg_if! {
        if #[cfg(any(target_arch = "x86", target_arch = "mips"))] {
            // On the 64-bit versions of these arches, there is no 32-bit compatibility code, so the
            // kernel writes out a 64-bit pointer. We need to handle this properly.

            let mut buf = 0u64;
            unsafe {
                crate::raw_prctl(libc::PR_GET_TID_ADDRESS, &mut buf as *mut _ as _, 0, 0, 0)?;
            }

            let addr = if cfg!(target_endian = "big") {
                // We have 2 possible cases:
                //
                // On real 32-bit systems (kernel writes 32-bit pointers):
                // 0xDEADBEEF_00000000
                //   ^^^^^^^^ ^^^^^^^^
                //   |        |
                //   |        zeroes from when we initialize `buf`
                //   |
                //   address (since the kernel only writes the first 32 bits)
                //
                // On 64-bit systems (kernel writes 64-bit pointers):
                // 0x00000000_DEADBEEF
                //   ^^^^^^^^ ^^^^^^^^
                //   |        |
                //   |        lower 32 bits (the real address)
                //   |
                //   upper 32 bits of the address (should be zero since we're a 32-bit process)

                if buf & 0xFFFF_FFFF == 0 {
                    // If the lower 32 bits are 0, take the upper 32 bits
                    (buf >> 32) as *mut libc::c_int
                } else {
                    // If the lower 32 bits are nonzero, take them
                    debug_assert_eq!(buf >> 32, 0, "address too large");
                    buf as *mut libc::c_int
                }
            } else {
                // We have 2 possible cases:
                //
                // On real 32-bit systems (kernel writes 32-bit pointers):
                // 0xFEEBDAED_00000000
                //   ^^^^^^^^ ^^^^^^^^
                //   |        |
                //   |        zeroes from when we initialize `buf`
                //   |
                //   address (since the kernel only writes the first 32 bits)
                //
                // On 64-bit systems (kernel writes 64-bit pointers):
                // 0xFEEBDAED_00000000
                //   ^^^^^^^^ ^^^^^^^^
                //   |        |
                //   |        upper 32 bits of the address (should be zero since we're 32-bit)
                //   |
                //   lower 32 bits (the real address)
                //
                // In both cases, we can just cast it to a 32-bit pointer

                debug_assert_eq!(buf >> 32, 0, "address too large");
                buf as *mut libc::c_int
            };
        } else {
            let mut addr = core::ptr::null_mut();
            unsafe {
                crate::raw_prctl(libc::PR_GET_TID_ADDRESS, &mut addr as *mut _ as _, 0, 0, 0)?;
            }
        }
    }

    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keepcaps() {
        let old_keepcaps = get_keepcaps().unwrap();

        set_keepcaps(true).unwrap();
        assert!(get_keepcaps().unwrap());
        assert!(get_securebits().unwrap().contains(Secbits::KEEP_CAPS));

        set_keepcaps(false).unwrap();
        assert!(!get_keepcaps().unwrap());
        assert!(!get_securebits().unwrap().contains(Secbits::KEEP_CAPS));

        set_keepcaps(old_keepcaps).unwrap();
    }

    #[test]
    fn test_nnp() {
        set_no_new_privs().unwrap();
        assert!(get_no_new_privs().unwrap());
        set_no_new_privs().unwrap();
        assert!(get_no_new_privs().unwrap());
    }

    #[test]
    fn test_subreaper() {
        let was_subreaper = get_subreaper().unwrap();

        set_subreaper(false).unwrap();
        assert!(!get_subreaper().unwrap());
        set_subreaper(true).unwrap();
        assert!(get_subreaper().unwrap());

        set_subreaper(was_subreaper).unwrap();
    }

    #[test]
    fn test_pdeathsig() {
        let orig_pdeathsig = get_pdeathsig().unwrap();

        set_pdeathsig(None).unwrap();
        assert_eq!(get_pdeathsig().unwrap(), None);
        set_pdeathsig(Some(0)).unwrap();
        assert_eq!(get_pdeathsig().unwrap(), None);

        set_pdeathsig(Some(libc::SIGCHLD)).unwrap();
        assert_eq!(get_pdeathsig().unwrap(), Some(libc::SIGCHLD));

        assert_eq!(set_pdeathsig(Some(-1)).unwrap_err().code(), libc::EINVAL);

        set_pdeathsig(orig_pdeathsig).unwrap();
    }

    #[test]
    fn test_dumpable() {
        assert!(get_dumpable().unwrap());
        // We can't set it to false because somebody may be ptrace()ing us during testing
        set_dumpable(true).unwrap();
        assert!(get_dumpable().unwrap());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_name() {
        let orig_name = get_name().unwrap();

        set_name("capctl-short").unwrap();
        assert_eq!(get_name().unwrap(), "capctl-short");

        set_name("capctl-very-very-long").unwrap();
        assert_eq!(get_name().unwrap(), "capctl-very-ver");

        assert_eq!(set_name("a\0").unwrap_err().code(), libc::EINVAL);

        set_name(&orig_name).unwrap();
        assert_eq!(get_name().unwrap(), orig_name);
    }

    #[test]
    fn test_securebits() {
        if crate::caps::CapState::get_current()
            .unwrap()
            .effective
            .has(crate::caps::Cap::SETPCAP)
        {
            let orig_secbits = get_securebits().unwrap();
            let mut secbits = orig_secbits;

            secbits.insert(Secbits::KEEP_CAPS);
            set_securebits(secbits).unwrap();
            assert!(get_keepcaps().unwrap());

            secbits.remove(Secbits::KEEP_CAPS);
            set_securebits(secbits).unwrap();
            assert!(!get_keepcaps().unwrap());

            set_securebits(orig_secbits).unwrap();
        } else {
            assert_eq!(
                set_securebits(get_securebits().unwrap())
                    .unwrap_err()
                    .code(),
                libc::EPERM
            );
        }
    }

    #[test]
    fn test_get_seccomp() {
        // We might be running in a Docker container or something with seccomp rules, so we can't
        // check the return value
        get_seccomp().unwrap();
    }

    #[test]
    fn test_set_seccomp_strict() {
        match unsafe { libc::fork() } {
            -1 => panic!("{}", crate::Error::last()),
            0 => {
                set_seccomp_strict().unwrap();

                unsafe {
                    libc::syscall(libc::SYS_exit, 0);
                    libc::_exit(1);
                }
            }
            pid => {
                let mut wstatus = 0;
                if unsafe { libc::waitpid(pid, &mut wstatus, 0) } != pid {
                    panic!("{}", crate::Error::last());
                }

                assert!(libc::WIFEXITED(wstatus));
                assert_eq!(libc::WEXITSTATUS(wstatus), 0);
            }
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_timerslack() {
        let orig_timerslack = get_timerslack().unwrap();
        set_timerslack(orig_timerslack + 1).unwrap();

        std::thread::spawn(move || {
            // The timer slack value is inherited
            assert_eq!(get_timerslack().unwrap(), orig_timerslack + 1);

            // We can change it
            set_timerslack(orig_timerslack).unwrap();
            assert_eq!(get_timerslack().unwrap(), orig_timerslack);

            // And if we set it to "0", it reverts to the "default" value inherited from the parent
            // thread
            set_timerslack(0).unwrap();
            assert_eq!(get_timerslack().unwrap(), orig_timerslack + 1);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_thp_disable() {
        let orig_thp_disable = get_thp_disable().unwrap();

        set_thp_disable(true).unwrap();
        assert!(get_thp_disable().unwrap());
        set_thp_disable(false).unwrap();
        assert!(!get_thp_disable().unwrap());

        set_thp_disable(orig_thp_disable).unwrap();
        assert_eq!(get_thp_disable().unwrap(), orig_thp_disable);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_ptracer() {
        // Invalid value; disallowed by our wrapper
        assert_eq!(
            set_ptracer(Ptracer::Pid(0)).unwrap_err().code(),
            libc::EINVAL
        );
        assert_eq!(
            set_ptracer(Ptracer::Pid(-1)).unwrap_err().code(),
            libc::EINVAL
        );

        if std::path::Path::new("/proc/sys/kernel/yama/ptrace_scope").exists() {
            // The Yama LSM is enabled, and set_ptracer() will actually work

            // Nonexistent process; denied by the Yama LSM
            assert_eq!(
                set_ptracer(Ptracer::Pid(libc::pid_t::MAX))
                    .unwrap_err()
                    .code(),
                libc::EINVAL
            );

            // Setting it to a real process works
            set_ptracer(Ptracer::Pid(1)).unwrap();
            set_ptracer(Ptracer::Pid(unsafe { libc::getppid() })).unwrap();
            set_ptracer(Ptracer::Pid(unsafe { libc::getpid() })).unwrap();

            // Clear it at the end
            set_ptracer(Ptracer::None).unwrap();
        } else {
            // Every call fails with EINVAL
            for &pid in unsafe { [libc::pid_t::MAX, 1, libc::getppid(), libc::getpid()] }.iter() {
                assert_eq!(
                    set_ptracer(Ptracer::Pid(pid)).unwrap_err().code(),
                    libc::EINVAL
                );
            }

            assert_eq!(set_ptracer(Ptracer::None).unwrap_err().code(), libc::EINVAL);
        }
    }

    #[test]
    fn test_mce_kill() {
        let orig_mce_kill = get_mce_kill().unwrap();

        set_mce_kill(MceKill::Early).unwrap();
        assert_eq!(get_mce_kill().unwrap(), MceKill::Early);
        set_mce_kill(MceKill::Late).unwrap();
        assert_eq!(get_mce_kill().unwrap(), MceKill::Late);
        set_mce_kill(MceKill::Default).unwrap();
        assert_eq!(get_mce_kill().unwrap(), MceKill::Default);

        set_mce_kill(orig_mce_kill).unwrap();
        assert_eq!(get_mce_kill().unwrap(), orig_mce_kill);
    }

    #[test]
    fn test_get_tid_address() {
        // We don't know for sure how the clear_child_tid address is being used, so we can't check
        // it
        get_tid_address().unwrap();
    }
}
