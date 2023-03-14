use super::{Cap, CapSet};

/// Drop the given capability from the current thread's bounding capability set.
#[inline]
pub fn drop(cap: Cap) -> crate::Result<()> {
    unsafe { crate::raw_prctl(libc::PR_CAPBSET_DROP, cap as libc::c_ulong, 0, 0, 0) }?;

    Ok(())
}

/// Check if the given capability is raised in the current thread's bounding capability set.
///
/// This returns `Some(true)` if the given capability is raised, `Some(false)` if it is lowered, and
/// `None` if it is not supported.
#[inline]
pub fn read(cap: Cap) -> Option<bool> {
    match unsafe { crate::raw_prctl_opt(libc::PR_CAPBSET_READ, cap as libc::c_ulong, 0, 0, 0) } {
        Some(res) => Some(res != 0),
        None => {
            #[cfg(not(feature = "sc"))]
            debug_assert_eq!(unsafe { *libc::__errno_location() }, libc::EINVAL);
            None
        }
    }
}

/// Check if the given capability is raised in the current thread's bounding capability set.
///
/// This is an alias of [`read()`](./fn.read.html).
#[inline]
pub fn is_set(cap: Cap) -> Option<bool> {
    read(cap)
}

/// "Probes" the current thread's bounding capability set and returns a `CapSet` representing all
/// the capabilities that are currently raised.
pub fn probe() -> CapSet {
    let mut set = CapSet::empty();

    for cap in Cap::iter() {
        match read(cap) {
            Some(true) => set.add(cap),
            Some(false) => (),

            // Unsupported capability encountered; none of the remaining ones will be supported
            // either
            _ => break,
        }
    }

    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding() {
        probe();
        is_set(Cap::CHOWN).unwrap();
    }

    #[test]
    fn test_bounding_drop() {
        if crate::caps::CapState::get_current()
            .unwrap()
            .effective
            .has(crate::caps::Cap::SETPCAP)
        {
            drop(crate::caps::Cap::SETPCAP).unwrap();
        } else {
            assert_eq!(
                drop(crate::caps::Cap::SETPCAP).unwrap_err().code(),
                libc::EPERM
            );
        }
    }
}
