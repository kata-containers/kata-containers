use super::{Cap, CapState};

/// Set the current thread's UID/GID/supplementary groups while preserving permitted capabilities.
///
/// This combines the functionality of ``libcap``'s ``cap_setuid()`` and ``cap_setgroups()``, while
/// providing greater flexibility.
///
/// WARNING: This function only operates on the current **thread**, not the process as a whole. This is
/// because of the way Linux operates. If you call this function from a multithreaded program, you
/// are responsible for synchronizing changes across threads as necessary to ensure proper security.
///
/// This function performs the following actions in order. (Note: If `gid` is not `None` or
/// `groups` is not `None`, CAP_SETGID will first be raised in the thread's effective set, and if
/// `uid` is not `None` then CAP_SETUID will be raised.)
///
/// - If `gid` is not `None`, the thread's real, effective and saved GIDs will be set to `gid`.
/// - If `groups` is not `None`, the thread's supplementary group list will be set to `groups`.
/// - If `uid` is not `None`, the thread's real, effective and saved UIDs will be set to `uid`.
/// - The effective capability set will be emptied.
///
/// Note: If this function fails and returns an error, the thread's UIDs, GIDs, supplementary
/// groups, and capability sets are in an unknown and possibly inconsistent state. This is EXTREMELY
/// DANGEROUS! If you are unable to revert the changes, abort as soon as possible.
pub fn cap_set_ids(
    uid: Option<libc::uid_t>,
    gid: Option<libc::gid_t>,
    groups: Option<&[libc::gid_t]>,
) -> crate::Result<()> {
    let mut capstate = CapState::get_current()?;
    let orig_effective = capstate.effective;

    let orig_keepcaps = crate::prctl::get_keepcaps()?;
    crate::prctl::set_keepcaps(true)?;

    if gid.is_some() || groups.is_some() {
        capstate.effective.add(Cap::SETGID);
    }
    if uid.is_some() {
        capstate.effective.add(Cap::SETUID);
    }

    if capstate.effective != orig_effective {
        if let Err(err) = capstate.set_current() {
            crate::prctl::set_keepcaps(orig_keepcaps)?;
            return Err(err);
        }
    }

    let res = do_set_ids(uid, gid, groups);

    // Now clear the effective capability set (if it wasn't already cleared) and restore the
    // "keepcaps" flag.
    capstate.effective.clear();
    res.and(capstate.set_current())
        .and(crate::prctl::set_keepcaps(orig_keepcaps))
}

cfg_if::cfg_if! {
    if #[cfg(all(
        target_pointer_width = "32",
        any(target_arch = "arm", target_arch = "sparc", target_arch = "x86")
    ))] {
        #[inline]
        unsafe fn setresuid(ruid: libc::uid_t, euid: libc::uid_t, suid: libc::uid_t) -> crate::Result<()> {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sc")] {
                    crate::sc_res_decode(sc::syscall!(SETRESUID32, ruid, euid, suid))?;
                } else {
                    if libc::syscall(libc::SYS_setresuid32, ruid, euid, suid) < 0 {
                        return Err(crate::Error::last());
                    }
                }
            }

            Ok(())
        }

        #[inline]
        unsafe fn setresgid(rgid: libc::gid_t, egid: libc::gid_t, sgid: libc::gid_t) -> crate::Result<()> {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sc")] {
                    crate::sc_res_decode(sc::syscall!(SETRESGID32, rgid, egid, sgid))?;
                } else {
                    if libc::syscall(libc::SYS_setresgid32, rgid, egid, sgid) < 0 {
                        return Err(crate::Error::last());
                    }
                }
            }

            Ok(())
        }

        #[inline]
        unsafe fn setgroups(size: libc::size_t, list: *const libc::gid_t) -> crate::Result<()> {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sc")] {
                    crate::sc_res_decode(sc::syscall!(SETGROUPS32, size, list))?;
                } else {
                    if libc::syscall(libc::SYS_setgroups32, size, list) < 0 {
                        return Err(crate::Error::last());
                    }
                }
            }

            Ok(())
        }
    } else {
        #[inline]
        unsafe fn setresuid(ruid: libc::uid_t, euid: libc::uid_t, suid: libc::uid_t) -> crate::Result<()> {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sc")] {
                    crate::sc_res_decode(sc::syscall!(SETRESUID, ruid, euid, suid))?;
                } else {
                    if libc::syscall(libc::SYS_setresuid, ruid, euid, suid) < 0 {
                        return Err(crate::Error::last());
                    }
                }
            }

            Ok(())
        }

        #[inline]
        unsafe fn setresgid(rgid: libc::gid_t, egid: libc::gid_t, sgid: libc::gid_t) -> crate::Result<()> {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sc")] {
                    crate::sc_res_decode(sc::syscall!(SETRESGID, rgid, egid, sgid))?;
                } else {
                    if libc::syscall(libc::SYS_setresgid, rgid, egid, sgid) < 0 {
                        return Err(crate::Error::last());
                    }
                }
            }

            Ok(())
        }

        #[inline]
        unsafe fn setgroups(size: libc::size_t, list: *const libc::gid_t) -> crate::Result<()> {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sc")] {
                    crate::sc_res_decode(sc::syscall!(SETGROUPS, size, list))?;
                } else {
                    if libc::syscall(libc::SYS_setgroups, size, list) < 0 {
                        return Err(crate::Error::last());
                    }
                }
            }

            Ok(())
        }
    }
}

fn do_set_ids(
    uid: Option<libc::uid_t>,
    gid: Option<libc::gid_t>,
    groups: Option<&[libc::gid_t]>,
) -> crate::Result<()> {
    unsafe {
        if let Some(gid) = gid {
            setresgid(gid, gid, gid)?;
        }

        if let Some(groups) = groups {
            setgroups(groups.len(), groups.as_ptr())?;
        }

        if let Some(uid) = uid {
            setresuid(uid, uid, uid)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_ids_none() {
        // All this does is clear the effective capability set
        cap_set_ids(None, None, None).unwrap();

        assert!(crate::caps::CapState::get_current()
            .unwrap()
            .effective
            .is_empty());
    }

    #[test]
    fn test_set_ids_some() {
        let permitted_caps = crate::caps::CapState::get_current().unwrap().permitted;

        let uid = unsafe { libc::geteuid() };
        let gid = unsafe { libc::getegid() };

        if permitted_caps.has(crate::caps::Cap::SETUID)
            && permitted_caps.has(crate::caps::Cap::SETGID)
        {
            cap_set_ids(Some(uid), Some(gid), None).unwrap();
        } else {
            assert_eq!(
                cap_set_ids(Some(uid), Some(gid), None).unwrap_err().code(),
                libc::EPERM
            );
        }
    }
}
