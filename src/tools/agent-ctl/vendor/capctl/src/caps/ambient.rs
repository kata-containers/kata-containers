use super::{Cap, CapSet};

/// Raise the given capability in the current thread's ambient capability set.
#[inline]
pub fn raise(cap: Cap) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_RAISE as libc::c_ulong,
            cap as libc::c_ulong,
            0,
            0,
        )
    }?;

    Ok(())
}

/// Lower the given capability in the current thread's ambient capability set.
#[inline]
pub fn lower(cap: Cap) -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_LOWER as libc::c_ulong,
            cap as libc::c_ulong,
            0,
            0,
        )
    }?;

    Ok(())
}

/// Check whether the given capability is raised in the current thread's ambient capability set.
///
/// This returns `Some(true)` if the given capability is raised, `Some(false)` if it is lowered,
/// and `None` if it is not supported.
#[inline]
pub fn is_set(cap: Cap) -> Option<bool> {
    match unsafe {
        crate::raw_prctl_opt(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_IS_SET as libc::c_ulong,
            cap as libc::c_ulong,
            0,
            0,
        )
    } {
        Some(res) => Some(res != 0),
        None => {
            #[cfg(not(feature = "sc"))]
            debug_assert_eq!(unsafe { *libc::__errno_location() }, libc::EINVAL);
            None
        }
    }
}

/// Clear the current thread's ambient capability set.
#[inline]
pub fn clear() -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL as libc::c_ulong,
            0,
            0,
            0,
        )
    }?;

    Ok(())
}

/// Check whether ambient capabilities are supported on the running kernel.
#[inline]
pub fn is_supported() -> bool {
    is_set(Cap::CHOWN).is_some()
}

/// "Probes" the current thread's ambient capability set and returns a `CapSet` representing all
/// the capabilities that are currently raised.
///
/// Returns `None` if ambient capabilities are not supported on the running kernel.
pub fn probe() -> Option<CapSet> {
    let mut set = CapSet::empty();

    for cap in Cap::iter() {
        match is_set(cap) {
            Some(true) => set.add(cap),
            Some(false) => (),

            // Unsupported capability encountered; none of the remaining ones will be supported
            // either
            None => {
                if cap as u8 == 0 {
                    // Ambient capabilities aren't supported at all
                    return None;
                } else {
                    break;
                }
            }
        }
    }

    Some(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ambient_supported() {
        if is_supported() {
            let orig_caps = probe().unwrap();
            let supported_caps = Cap::probe_supported();

            // Make sure everything is consistent
            for cap in Cap::iter() {
                if orig_caps.has(cap) {
                    assert_eq!(is_set(cap), Some(true));
                } else if supported_caps.has(cap) {
                    assert_eq!(is_set(cap), Some(false));
                } else {
                    assert_eq!(is_set(cap), None);
                }
            }

            // Clear the ambient capability set
            clear().unwrap();

            // Now make sure it's actually empty
            assert_eq!(probe().unwrap(), CapSet::empty());

            // Raise all the capabilities that were in there originally
            for cap in orig_caps.iter() {
                raise(cap).unwrap();
            }

            // Lower all the ones that weren't
            for cap in (supported_caps - orig_caps).iter() {
                lower(cap).unwrap();
            }

            for cap in !supported_caps {
                assert_eq!(raise(cap).unwrap_err().code(), libc::EINVAL);
                assert_eq!(lower(cap).unwrap_err().code(), libc::EINVAL);
            }
        } else {
            assert_eq!(probe(), None);
            assert_eq!(raise(Cap::CHOWN).unwrap_err().code(), libc::EINVAL);
            assert_eq!(lower(Cap::CHOWN).unwrap_err().code(), libc::EINVAL);
            assert_eq!(clear().unwrap_err().code(), libc::EINVAL);
        }
    }
}
