use super::{Cap, CapSet};

/// Drop the given capability from the current thread's bounding capability set.
///
/// Note that this will fail with `EPERM` if the current thread does not have `CAP_SETPCAP`, even
/// if the given capability is already lowered. Callers may wish to use [`ensure_dropped()`]
/// instead.
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
    read_raw(cap as _)
}

#[inline]
fn read_raw(cap: libc::c_ulong) -> Option<bool> {
    match unsafe { crate::raw_prctl_opt(libc::PR_CAPBSET_READ, cap, 0, 0, 0) } {
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
#[deprecated(since = "0.2.1", note = "use `read()` instead")]
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

/// Try to ensure that the given capability is dropped.
///
/// This is a helper that first tries to drop the capability. If that fails with `EPERM`, it
/// `read()`s the capability to see if it's already lowered.
///
/// This function will:
///
/// - Return `Ok(())` if the capability is now lowered.
/// - Fail with `EPERM` if the capability is raised and the current thread does not have
///   `CAP_SETPCAP`.
/// - Fail with `EINVAL` if the capability is not supported by the kernel.
pub fn ensure_dropped(cap: Cap) -> crate::Result<()> {
    ensure_dropped_raw(cap as _)
}

#[inline]
fn ensure_dropped_raw(cap: libc::c_ulong) -> crate::Result<()> {
    match unsafe { crate::raw_prctl(libc::PR_CAPBSET_DROP, cap, 0, 0, 0) } {
        // Successfully lowered the capability
        Ok(_) => Ok(()),

        // EPERM -> we don't have CAP_SETPCAP
        // Check the current setting of the capability to decide what to do
        Err(e) if e.code() == libc::EPERM => match read_raw(cap) {
            // The capability is raised -> pass up EPERM to the caller
            Some(true) => Err(e),
            // The capability is lowered -> we have nothing to do
            Some(false) => Ok(()),
            // The capability is unsupported
            None => Err(crate::Error::from_code(libc::EINVAL)),
        },

        // Pass all other errors up to the caller
        Err(e) => Err(e),
    }
}

fn clear_from(low: libc::c_ulong) -> crate::Result<()> {
    for cap in low..(super::CAP_MAX as libc::c_ulong * 2) {
        match ensure_dropped_raw(cap) {
            Ok(()) => (),
            // Unknown capability
            // If cap is not 0, we found the last capability
            Err(e) if e.code() == libc::EINVAL && cap != 0 => return Ok(()),
            // Pass all other errors up to the caller
            Err(e) => return Err(e),
        }
    }

    Err(crate::Error::from_code(libc::E2BIG))
}

/// Drop all capabilities supported by the kernel from the current thread's bounding capability set.
///
/// This method is roughly equivalent to the following (though it may be slightly faster):
///
/// ```no_run
/// # use capctl::*;
/// # fn clear() -> Result<()> {
/// for cap in Cap::iter() {
///     if bounding::read(cap) == Some(true) {
///         bounding::drop(cap)?;
///     }
/// }
/// bounding::clear_unknown()?;
/// # Ok(())
/// # }
/// ```
///
/// The intent is to simulate [`crate::caps::ambient::clear()`] (which is a single `prctl()` call).
///
/// See also [`clear_unknown()`].
#[inline]
pub fn clear() -> crate::Result<()> {
    clear_from(0)
}

/// Drop all capabilities that are supported by the kernel but which this library is not aware of
/// from the current thread's bounding capability set.
///
/// For example, this code will drop all bounding capabilities (even ones not supported by
/// `capctl`) except for `CAP_SETUID`:
///
/// ```no_run
/// # use capctl::*;
/// // Drop all capabilities that `capctl` knows about (except for CAP_SETUID)
/// for cap in Cap::iter() {
///     if cap != Cap::SETUID {
///         bounding::drop(cap).unwrap();
///     }
/// }
/// // Drop any new capabilities that `capctl` wasn't aware of at compile time
/// bounding::clear_unknown();
/// ```
///
/// See [Handling of newly-added capabilities](../index.html#handling-of-newly-added-capabilities)
/// for the rationale.
#[inline]
pub fn clear_unknown() -> crate::Result<()> {
    clear_from((super::CAP_MAX + 1) as _)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding() {
        probe();
        read(Cap::CHOWN).unwrap();
    }

    #[test]
    fn test_bounding_drop() {
        if crate::caps::CapState::get_current()
            .unwrap()
            .effective
            .has(crate::caps::Cap::SETPCAP)
        {
            assert!(read(crate::caps::Cap::SETPCAP).unwrap());
            drop(crate::caps::Cap::SETPCAP).unwrap();
            assert!(!read(crate::caps::Cap::SETPCAP).unwrap());
        } else {
            assert_eq!(
                drop(crate::caps::Cap::SETPCAP).unwrap_err().code(),
                libc::EPERM
            );
        }
    }

    #[test]
    fn test_clear() {
        let mut state = crate::caps::CapState::get_current().unwrap();
        if state.effective.has(crate::caps::Cap::SETPCAP) || probe().is_empty() {
            clear().unwrap();
            assert_eq!(probe(), crate::caps::CapSet::empty());
            clear().unwrap();
            assert_eq!(probe(), crate::caps::CapSet::empty());

            state.effective.drop(crate::caps::Cap::SETPCAP);
            state.set_current().unwrap();
            clear().unwrap();
            assert_eq!(probe(), crate::caps::CapSet::empty());
        } else {
            assert_eq!(clear().unwrap_err().code(), libc::EPERM);
        }
    }

    #[test]
    fn test_clear_unknown() {
        let mut state = crate::caps::CapState::get_current().unwrap();
        if state.effective.has(crate::caps::Cap::SETPCAP)
            || read_raw((super::super::CAP_MAX + 1) as _).is_none()
        {
            // Either we have CAP_SETPCAP, or there are no unknown capabilities
            let orig_caps = probe();

            clear_unknown().unwrap();
            assert_eq!(probe(), orig_caps);
            clear_unknown().unwrap();
            assert_eq!(probe(), orig_caps);

            // If there are unknown capabilities, the first one is NOT raised in the bounding set
            assert!(matches!(
                read_raw((super::super::CAP_MAX + 1) as _),
                Some(false) | None
            ));

            state.effective.drop(crate::caps::Cap::SETPCAP);
            state.set_current().unwrap();
            clear_unknown().unwrap();
            assert_eq!(probe(), orig_caps);
        } else {
            assert_eq!(clear_unknown().unwrap_err().code(), libc::EPERM);
        }
    }
}
