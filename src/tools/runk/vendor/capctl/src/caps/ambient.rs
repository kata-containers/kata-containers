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
        )?;
    }

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
        )?;
    }

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
///
/// This is a single `prctl()` call (`PR_CAP_AMBIENT_CLEAR_ALL`) that removes all capabilities
/// supported by the kernel from the ambient set.
#[inline]
pub fn clear() -> crate::Result<()> {
    unsafe {
        crate::raw_prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL as libc::c_ulong,
            0,
            0,
            0,
        )?;
    }

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

/// Drop all bounding capabilities that are supported by the kernel but which this library is not
/// aware of from the current thread's ambient capability set.
///
/// See [Handling of newly-added capabilities](../index.html#handling-of-newly-added-capabilities)
/// for the rationale.
pub fn clear_unknown() -> crate::Result<()> {
    for cap in (super::CAP_MAX as libc::c_ulong + 1)..(super::CAP_MAX as libc::c_ulong * 2) {
        match unsafe {
            crate::raw_prctl(
                libc::PR_CAP_AMBIENT,
                libc::PR_CAP_AMBIENT_LOWER as libc::c_ulong,
                cap,
                0,
                0,
            )
        } {
            Ok(_) => (),
            Err(e) if e.code() == libc::EINVAL => return Ok(()),
            Err(e) => return Err(e),
        }
    }

    Err(crate::Error::from_code(libc::E2BIG))
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
                if supported_caps.has(cap) {
                    assert_eq!(is_set(cap), Some(orig_caps.has(cap)), "{:?}", cap);
                } else {
                    assert_eq!(is_set(cap), None, "{:?}", cap);
                }
            }

            // Clear unknown capabilities from the ambient capability set
            clear_unknown().unwrap();
            assert_eq!(probe().unwrap(), orig_caps);

            // Clear the ambient capability set
            clear().unwrap();

            // Now make sure it's actually empty
            assert_eq!(probe().unwrap(), CapSet::empty());

            // Now test actually raising capabilities
            let orig_state = crate::caps::CapState::get_current().unwrap();
            let mut state = orig_state;
            // To start, copy the permitted set to the inheritable set
            state.inheritable = state.permitted;
            state.set_current().unwrap();

            // Now raise all of those capabilities in the ambient set
            for cap in state.inheritable {
                raise(cap).unwrap();
            }
            // Now clear the inheritable set
            state.inheritable.clear();
            state.set_current().unwrap();
            // The ambient set should be automatically cleared
            assert_eq!(probe().unwrap(), CapSet::empty());
            // And trying to raise any capability in the ambient set will now fail
            for cap in supported_caps {
                assert_eq!(raise(cap).unwrap_err().code(), libc::EPERM);
            }

            // Restore the original capability state at the end
            orig_state.set_current().unwrap();

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
            assert_eq!(clear_unknown().unwrap_err().code(), libc::EINVAL);
        }
    }
}
