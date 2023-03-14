use core::fmt;

use crate::sys;

use super::cap_text::{caps_from_text, caps_to_text, ParseCapsError};
use super::CapSet;

/// Represents the permitted, effective, and inheritable capability sets of a thread.
///
/// # `FromStr` and `Display` implementations
///
/// This struct's implementations of  `FromStr` and `Display` use the same format as `libcap`'s
/// `cap_from_text()` and `cap_to_text()`. For example, an empty state can be represented as `=`, a
/// "full" state can be represented as `=eip`, and a state containing only `CAP_CHOWN` in the
/// effective and permitted sets can be represented by `cap_chown=ep`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct CapState {
    pub effective: CapSet,
    pub permitted: CapSet,
    pub inheritable: CapSet,
}

impl CapState {
    /// Construct an empty `CapState` object.
    #[inline]
    pub fn empty() -> Self {
        Self {
            effective: CapSet::empty(),
            permitted: CapSet::empty(),
            inheritable: CapSet::empty(),
        }
    }

    /// Get the capability state of the current thread.
    ///
    /// This is equivalent to `CapState::get_for_pid(0)`.
    #[inline]
    pub fn get_current() -> crate::Result<Self> {
        Self::get_for_pid(0)
    }

    /// Get the capability state of the process (or thread) with the given PID (or TID).
    ///
    /// If `pid` is 0, this method gets the capability state of the current thread.
    pub fn get_for_pid(pid: libc::pid_t) -> crate::Result<Self> {
        let mut header = sys::cap_user_header_t {
            version: sys::_LINUX_CAPABILITY_VERSION_3,
            pid: pid as libc::c_int,
        };

        let mut raw_dat = [sys::cap_user_data_t {
            effective: 0,
            permitted: 0,
            inheritable: 0,
        }; 2];

        cfg_if::cfg_if! {
            if #[cfg(feature = "sc")] {
                crate::sc_res_decode(unsafe {
                    sc::syscall!(CAPGET, &mut header as *mut _, raw_dat.as_mut_ptr())
                })?;
            } else {
                if unsafe { sys::capget(&mut header, raw_dat.as_mut_ptr()) } < 0 {
                    return Err(crate::Error::last());
                }
            }
        }

        Ok(Self {
            effective: CapSet::from_bitmasks_u32(raw_dat[0].effective, raw_dat[1].effective),
            permitted: CapSet::from_bitmasks_u32(raw_dat[0].permitted, raw_dat[1].permitted),
            inheritable: CapSet::from_bitmasks_u32(raw_dat[0].inheritable, raw_dat[1].inheritable),
        })
    }

    /// Set the current capability state to the state represented by this object.
    pub fn set_current(&self) -> crate::Result<()> {
        let mut header = sys::cap_user_header_t {
            version: sys::_LINUX_CAPABILITY_VERSION_3,
            pid: 0,
        };

        let effective = self.effective.bits;
        let permitted = self.permitted.bits;
        let inheritable = self.inheritable.bits;

        let raw_dat = [
            sys::cap_user_data_t {
                effective: effective as u32,
                permitted: permitted as u32,
                inheritable: inheritable as u32,
            },
            sys::cap_user_data_t {
                effective: (effective >> 32) as u32,
                permitted: (permitted >> 32) as u32,
                inheritable: (inheritable >> 32) as u32,
            },
        ];

        cfg_if::cfg_if! {
            if #[cfg(feature = "sc")] {
                crate::sc_res_decode(unsafe {
                    sc::syscall!(CAPSET, &mut header as *mut _, raw_dat.as_ptr())
                })?;
            } else {
                if unsafe { sys::capset(&mut header, raw_dat.as_ptr()) } < 0 {
                    return Err(crate::Error::last());
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for CapState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        caps_to_text(*self, f)
    }
}

impl core::str::FromStr for CapState {
    type Err = ParseCapStateError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        caps_from_text(s).map_err(ParseCapStateError)
    }
}

/// Represents an error when parsing a `CapState` object from a string.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ParseCapStateError(ParseCapsError);

impl fmt::Display for ParseCapStateError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[cfg(feature = "std")]
impl std::error::Error for ParseCapStateError {}

#[cfg(test)]
mod tests {
    use super::*;

    use core::str::FromStr;

    use crate::caps::Cap;
    use crate::capset;

    #[test]
    fn test_capstate_empty() {
        assert_eq!(
            CapState::empty(),
            CapState {
                effective: CapSet::empty(),
                permitted: CapSet::empty(),
                inheritable: CapSet::empty(),
            }
        );
    }

    #[test]
    fn test_capstate_getset_current() {
        let state = CapState::get_current().unwrap();
        assert_eq!(state, CapState::get_for_pid(0).unwrap());
        assert_eq!(
            state,
            CapState::get_for_pid(unsafe { libc::getpid() }).unwrap()
        );
        state.set_current().unwrap();
    }

    #[test]
    fn test_capstate_get_bad_pid() {
        assert_eq!(CapState::get_for_pid(-1).unwrap_err().code(), libc::EINVAL);
        assert_eq!(
            CapState::get_for_pid(libc::pid_t::MAX).unwrap_err().code(),
            libc::ESRCH
        );
    }

    #[test]
    fn test_capstate_parse() {
        // caps_from_text() has more extensive tests; we can be a little loose here

        assert_eq!(
            CapState::from_str("cap_chown=eip cap_chown-p cap_syslog+p").unwrap(),
            CapState {
                permitted: capset!(Cap::SYSLOG),
                effective: capset!(Cap::CHOWN),
                inheritable: capset!(Cap::CHOWN),
            }
        );

        #[cfg(feature = "std")]
        assert_eq!(
            CapState::from_str("cap_noexist+p").unwrap_err().to_string(),
            "Unknown capability"
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_capstate_display() {
        // caps_to_text() has no tests in cap_text.rs, so we need to be rigorous

        assert_eq!(CapState::empty().to_string(), "=");

        assert_eq!(
            CapState {
                permitted: !capset!(),
                effective: !capset!(),
                inheritable: capset!(),
            }
            .to_string(),
            "=ep",
        );

        assert_eq!(
            CapState {
                permitted: capset!(Cap::CHOWN),
                effective: capset!(Cap::CHOWN),
                inheritable: capset!(Cap::CHOWN),
            }
            .to_string(),
            "cap_chown=eip",
        );

        assert_eq!(
            CapState {
                permitted: capset!(Cap::CHOWN),
                effective: capset!(Cap::CHOWN),
                inheritable: capset!(),
            }
            .to_string(),
            "cap_chown=ep",
        );

        assert_eq!(
            CapState {
                permitted: !capset!(Cap::CHOWN),
                effective: !capset!(Cap::CHOWN),
                inheritable: capset!(),
            }
            .to_string(),
            "=ep cap_chown-ep",
        );

        for state in [
            CapState::empty(),
            CapState {
                permitted: !capset!(),
                effective: !capset!(),
                inheritable: !capset!(),
            },
            CapState {
                permitted: !capset!(),
                effective: capset!(),
                inheritable: capset!(),
            },
            CapState {
                permitted: !capset!(Cap::CHOWN),
                effective: capset!(),
                inheritable: capset!(),
            },
            CapState {
                permitted: capset!(),
                effective: !capset!(Cap::CHOWN),
                inheritable: capset!(),
            },
            CapState {
                permitted: capset!(),
                effective: capset!(),
                inheritable: !capset!(Cap::CHOWN),
            },
            CapState {
                permitted: !capset!(Cap::CHOWN),
                effective: capset!(Cap::CHOWN),
                inheritable: capset!(),
            },
            CapState {
                permitted: capset!(Cap::CHOWN),
                effective: capset!(Cap::CHOWN),
                inheritable: capset!(Cap::CHOWN),
            },
            CapState {
                permitted: capset!(Cap::SYSLOG),
                effective: capset!(Cap::CHOWN),
                inheritable: capset!(Cap::CHOWN),
            },
            CapState {
                permitted: capset!(Cap::SYSLOG, Cap::CHOWN),
                effective: capset!(Cap::CHOWN),
                inheritable: capset!(Cap::CHOWN),
            },
            CapState {
                permitted: capset!(Cap::SYSLOG, Cap::CHOWN),
                effective: capset!(Cap::SYSLOG, Cap::CHOWN),
                inheritable: capset!(Cap::SYSLOG, Cap::CHOWN),
            },
            CapState {
                permitted: capset!(),
                effective: capset!(),
                inheritable: capset!(Cap::SYSLOG, Cap::CHOWN),
            },
            // Let's try some real-world data
            CapState::get_current().unwrap(),
            CapState::get_for_pid(1).unwrap(),
        ]
        .iter()
        {
            let s = state.to_string();

            assert_eq!(s.parse::<CapState>().unwrap(), *state, "{:?}", s);
        }
    }
}
