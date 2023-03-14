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

    unsafe { crate::raw_prctl(libc::PR_SET_NAME, ptr as libc::c_ulong, 0, 0, 0) }?;

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
        )
    }?;

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
    unsafe { crate::raw_prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) }?;

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
    unsafe { crate::raw_prctl(libc::PR_SET_KEEPCAPS, keep as libc::c_ulong, 0, 0, 0) }?;

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
    unsafe { crate::raw_prctl(libc::PR_SET_DUMPABLE, dumpable as libc::c_ulong, 0, 0, 0) }?;

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
    unsafe { crate::raw_prctl(libc::PR_SET_CHILD_SUBREAPER, flag as libc::c_ulong, 0, 0, 0) }?;

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
        )
    }?;

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
        )
    }?;

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
        )
    }?;

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
        /// using this flag. As a result, this flag is mainly useful for locking the `KEEP_CAPS` in
        /// the "off" setting.
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
pub fn set_securebits(flags: Secbits) -> crate::Result<()> {
    unsafe { crate::raw_prctl(libc::PR_SET_SECUREBITS, flags.bits(), 0, 0, 0) }?;

    Ok(())
}

/// Get the secure computing mode of the current thread.
///
/// If the thread is not in secure computing mode, this function returns `false`; if it is in
/// seccomp filter mode (and the `prctl()` syscall with the given arguments is allowed by the
/// filters) then this function returns `true`; if it is in strict computing mode then it will be
/// sent a SIGKILL signal.
pub fn get_seccomp() -> crate::Result<bool> {
    let res = unsafe { crate::raw_prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) }?;

    Ok(res != 0)
}

/// Enable strict secure computing mode.
///
/// After this call, any syscalls except `read()`, `write()`, `_exit()`, and `sigreturn()` will
/// cause the thread to be terminated with SIGKILL.
pub fn set_seccomp_strict() -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_STRICT as libc::c_ulong,
            0,
            0,
            0,
        )
    }?;

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
pub fn get_timerslack() -> crate::Result<libc::c_ulong> {
    #[cfg(not(feature = "sc"))]
    return {
        let res = unsafe { libc::syscall(libc::SYS_prctl, libc::PR_GET_TIMERSLACK, 0, 0, 0) };

        if res == -1 {
            Err(crate::Error::last())
        } else {
            Ok(res as libc::c_ulong)
        }
    };

    #[cfg(feature = "sc")]
    return crate::sc_res_decode(unsafe { sc::syscall!(PRCTL, libc::PR_GET_TIMERSLACK, 0, 0, 0) })
        .map(|res| res as libc::c_ulong);
}

/// Set the current timer slack value.
///
/// The timer slack value is used by the kernel to group timer expirations (`select()`,
/// `epoll_wait()`, `nanosleep()`, etc.) for the calling thread. See prctl(2) for more details.
///
/// Note: Passing a value of 0 will reset the current timer slack value to the "default" timer
/// slack value (which is inherited from the parent). Again, prctl(2) contains more information.
pub fn set_timerslack(new_slack: libc::c_ulong) -> crate::Result<()> {
    unsafe { crate::raw_prctl(libc::PR_SET_TIMERSLACK, new_slack, 0, 0, 0) }?;

    Ok(())
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
                    libc::exit(0);
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
}
