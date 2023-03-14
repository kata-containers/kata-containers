use core::fmt;

mod cap_text;
mod capset;
mod capstate;
mod helpers;

#[cfg(feature = "serde")]
mod serde_impl;

#[cfg(feature = "std")]
mod file;
#[cfg(feature = "std")]
pub use file::{FileCaps, ParseFileCapsError};

#[cfg(feature = "std")]
mod fullcapstate;
#[cfg(feature = "std")]
pub use fullcapstate::FullCapState;

pub mod ambient;
pub mod bounding;
pub use capset::{CapSet, CapSetIterator};
pub use capstate::{CapState, ParseCapStateError};
pub use helpers::cap_set_ids;

/// An enum representing all of the possible Linux capabilities.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
#[allow(non_camel_case_types)]
#[non_exhaustive]
pub enum Cap {
    CHOWN = 0,
    DAC_OVERRIDE = 1,
    DAC_READ_SEARCH = 2,
    FOWNER = 3,
    FSETID = 4,
    KILL = 5,
    SETGID = 6,
    SETUID = 7,
    SETPCAP = 8,
    LINUX_IMMUTABLE = 9,
    NET_BIND_SERVICE = 10,
    NET_BROADCAST = 11,
    NET_ADMIN = 12,
    NET_RAW = 13,
    IPC_LOCK = 14,
    IPC_OWNER = 15,
    SYS_MODULE = 16,
    SYS_RAWIO = 17,
    SYS_CHROOT = 18,
    SYS_PTRACE = 19,
    SYS_PACCT = 20,
    SYS_ADMIN = 21,
    SYS_BOOT = 22,
    SYS_NICE = 23,
    SYS_RESOURCE = 24,
    SYS_TIME = 25,
    SYS_TTY_CONFIG = 26,
    MKNOD = 27,
    LEASE = 28,
    AUDIT_WRITE = 29,
    AUDIT_CONTROL = 30,
    SETFCAP = 31,
    MAC_OVERRIDE = 32,
    MAC_ADMIN = 33,
    SYSLOG = 34,
    WAKE_ALARM = 35,
    BLOCK_SUSPEND = 36,
    AUDIT_READ = 37,
    PERFMON = 38,
    BPF = 39,
    CHECKPOINT_RESTORE = 40,
    // Note: When adding a new capability, make sure to update LAST_CAP and CAP_NAMES
}

// *** WARNING WARNING WARNING ***
// This MUST be set to the last capability from the above list!
// This assumption is used by unsafe code to speed up checks.
const LAST_CAP: Cap = Cap::CHECKPOINT_RESTORE;

// Some other useful values derived from LAST_CAP
const CAP_MAX: u8 = LAST_CAP as u8;
const NUM_CAPS: u8 = CAP_MAX + 1;
// Shift to the left, then subtract one to get the lower bits filled with ones.
const CAP_BITMASK: u64 = (1u64 << NUM_CAPS) - 1;

static CAP_NAMES: [&str; NUM_CAPS as usize] = [
    "CHOWN",
    "DAC_OVERRIDE",
    "DAC_READ_SEARCH",
    "FOWNER",
    "FSETID",
    "KILL",
    "SETGID",
    "SETUID",
    "SETPCAP",
    "LINUX_IMMUTABLE",
    "NET_BIND_SERVICE",
    "NET_BROADCAST",
    "NET_ADMIN",
    "NET_RAW",
    "IPC_LOCK",
    "IPC_OWNER",
    "SYS_MODULE",
    "SYS_RAWIO",
    "SYS_CHROOT",
    "SYS_PTRACE",
    "SYS_PACCT",
    "SYS_ADMIN",
    "SYS_BOOT",
    "SYS_NICE",
    "SYS_RESOURCE",
    "SYS_TIME",
    "SYS_TTY_CONFIG",
    "MKNOD",
    "LEASE",
    "AUDIT_WRITE",
    "AUDIT_CONTROL",
    "SETFCAP",
    "MAC_OVERRIDE",
    "MAC_ADMIN",
    "SYSLOG",
    "WAKE_ALARM",
    "BLOCK_SUSPEND",
    "AUDIT_READ",
    "PERFMON",
    "BPF",
    "CHECKPOINT_RESTORE",
];

impl Cap {
    /// Return an iterator over all of the capabilities enumerated by `Cap`.
    #[inline]
    pub fn iter() -> CapIter {
        CapIter { i: 0 }
    }

    #[inline]
    fn from_u8(val: u8) -> Option<Self> {
        if val <= CAP_MAX {
            Some(unsafe { core::mem::transmute(val) })
        } else {
            None
        }
    }

    #[inline]
    fn to_single_bitfield(self) -> u64 {
        // Sanity check to help ensure CAP_MAX is set correctly (note that this will only catch some
        // cases)
        debug_assert!((self as u8) <= CAP_MAX);

        1u64 << (self as u8)
    }

    /// Checks whether the specified capability is supported on the current kernel.
    pub fn is_supported(self) -> bool {
        bounding::read(self).is_some()
    }

    /// Determines the set of capabilities supported by the running kernel.
    ///
    /// This uses a binary search combined with [`Cap::is_supported()`] to determine the supported
    /// capabilities. It is more efficient than a simple `Cap::iter()`/`Cap::is_supported()` loop.
    ///
    /// [`Cap::is_supported()`]: #method.is_supported
    pub fn probe_supported() -> CapSet {
        // Do a binary search

        // Rust currently supports kernel 2.6.32+. CAP_MAC_ADMIN was the last capability added
        // before that release (in kernel 2.6.25).
        let mut min = Self::MAC_ADMIN as u8;
        let mut max = CAP_MAX;

        debug_assert!(Self::from_u8(min).unwrap().is_supported());

        while min != max {
            // This basically does `mid = ceil((min + max) / 2)`.
            // If we don't do ceiling division, the way binary search works, we'll get stuck at
            // `max = min + 1` forever.
            let sum = min + max;
            let mid = (sum >> 1) + (sum & 1);

            if Self::from_u8(mid).unwrap().is_supported() {
                min = mid;
            } else {
                max = mid - 1;
            }

            debug_assert!(max >= min);
        }

        CapSet::from_bitmask_truncate((1 << (min + 1)) - 1)
    }

    pub(crate) fn name(self) -> &'static str {
        CAP_NAMES[self as usize]
    }
}

impl core::str::FromStr for Cap {
    type Err = ParseCapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() > 4 && s[..4].eq_ignore_ascii_case("CAP_") {
            let s = &s[4..];

            for (i, cap_name) in CAP_NAMES.iter().enumerate() {
                if cap_name.eq_ignore_ascii_case(s) {
                    debug_assert!(Cap::from_u8(i as u8).is_some());

                    return Ok(unsafe { core::mem::transmute(i as u8) });
                }
            }
        }

        Err(ParseCapError(()))
    }
}

impl fmt::Display for Cap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CAP_")?;
        fmt::Debug::fmt(self, f)
    }
}

/// Represents an error when parsing a `Cap` from a string.
#[derive(Clone, Eq, PartialEq)]
pub struct ParseCapError(());

impl fmt::Debug for ParseCapError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ParseCapError")
            .field("message", &"Unknown capability")
            .finish()
    }
}

impl fmt::Display for ParseCapError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Unknown capability")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseCapError {}

/// An iterator over all the capabilities enumerated in `Cap`.
///
/// This is constructed by [`Cap::iter()`].
///
/// [`Cap::iter()`]: ./enum.Cap.html#method.iter
#[derive(Clone)]
pub struct CapIter {
    i: u8,
}

impl Iterator for CapIter {
    type Item = Cap;

    fn next(&mut self) -> Option<Cap> {
        debug_assert!(self.i <= NUM_CAPS);

        let cap = Cap::from_u8(self.i)?;
        self.i += 1;
        Some(cap)
    }

    fn nth(&mut self, n: usize) -> Option<Cap> {
        if n < self.len() {
            self.i += n as u8;
            self.next()
        } else {
            // The specified index would exhaust the iterator
            self.i = NUM_CAPS;
            None
        }
    }

    #[inline]
    fn last(self) -> Option<Cap> {
        if self.i < NUM_CAPS {
            Some(LAST_CAP)
        } else {
            None
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for CapIter {
    #[inline]
    fn len(&self) -> usize {
        (NUM_CAPS - self.i) as usize
    }
}

impl core::iter::FusedIterator for CapIter {}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::*;

    #[test]
    fn test_cap_u8() {
        for i in 0..NUM_CAPS {
            assert_eq!(Cap::from_u8(i).unwrap() as u8, i);
        }

        assert_eq!(Cap::from_u8(NUM_CAPS), None);
    }

    #[test]
    fn test_cap_string() {
        assert_eq!(Cap::from_str("CAP_CHOWN"), Ok(Cap::CHOWN));
        assert_eq!(Cap::from_str("cap_CHOWN"), Ok(Cap::CHOWN));
        assert_eq!(Cap::from_str("Cap_CHOWN"), Ok(Cap::CHOWN));

        assert_eq!(Cap::from_str("CAP_sys_chroot"), Ok(Cap::SYS_CHROOT));
        assert_eq!(Cap::from_str("cap_sys_chroot"), Ok(Cap::SYS_CHROOT));
        assert_eq!(Cap::from_str("Cap_Sys_chroot"), Ok(Cap::SYS_CHROOT));

        assert!(Cap::from_str("").is_err());
        assert!(Cap::from_str("CAP_").is_err());
        assert!(Cap::from_str("CHOWN").is_err());
        assert!(Cap::from_str("CAP_NOEXIST").is_err());

        #[cfg(feature = "std")]
        assert_eq!(Cap::CHOWN.to_string(), "CAP_CHOWN");

        #[cfg(feature = "std")]
        for cap in Cap::iter() {
            let s = cap.to_string();
            assert_eq!(Cap::from_str(&s), Ok(cap));
            assert_eq!(Cap::from_str(&s.to_lowercase()), Ok(cap));
            assert_eq!(Cap::from_str(&s.to_uppercase()), Ok(cap));
        }

        for (cap, name) in Cap::iter().zip(&CAP_NAMES) {
            // Concatenate strings without allocating
            let mut full_name = [0u8; 30];
            full_name[..4].copy_from_slice(b"cap_");
            full_name[4..name.len() + 4].copy_from_slice(name.as_bytes());

            assert_eq!(
                Cap::from_str(core::str::from_utf8(&full_name[..name.len() + 4]).unwrap()),
                Ok(cap)
            );
        }
    }

    #[cfg(feature = "std")]
    #[allow(deprecated)]
    #[test]
    fn test_cap_string_error() {
        let err = ParseCapError(());

        // Make sure clone() and eq() work
        // This will probably be optimized away because it's zero-sized, but it checks that the
        // struct derives Clone and Eq.
        assert_eq!(err, err.clone());

        // Make sure the string representations match
        assert_eq!(err.to_string(), "Unknown capability");
        assert_eq!(
            format!("{:?}", err),
            "ParseCapError { message: \"Unknown capability\" }"
        );
    }

    #[test]
    fn test_cap_iter_last() {
        assert_eq!(Cap::iter().last(), Some(LAST_CAP));

        let mut last = None;
        for cap in Cap::iter() {
            last = Some(cap);
        }
        assert_eq!(last, Some(LAST_CAP));

        let mut it = Cap::iter();
        for _ in it.by_ref() {}
        assert_eq!(it.len(), 0);
        assert_eq!(it.last(), None);
    }

    #[allow(clippy::iter_nth_zero)]
    #[test]
    fn test_cap_iter_nth() {
        let mut it = Cap::iter();
        while let Some(cap) = it.clone().next() {
            assert_eq!(cap, it.nth(0).unwrap());
        }
        assert_eq!(it.nth(0), None);

        assert_eq!(Cap::iter().nth(0), Some(Cap::CHOWN));
        assert_eq!(Cap::iter().nth(1), Some(Cap::DAC_OVERRIDE));
        assert_eq!(Cap::iter().nth(NUM_CAPS as usize - 1), Some(LAST_CAP));
    }

    #[allow(clippy::iter_nth_zero)]
    #[test]
    fn test_cap_iter_fused() {
        let mut it = Cap::iter();
        for _ in it.by_ref() {}

        for _ in 0..256 {
            assert_eq!(it.next(), None);
            assert_eq!(it.nth(0), None);
        }
    }

    #[test]
    fn test_cap_iter_count() {
        let mut it = Cap::iter();

        let mut count = it.len();

        assert_eq!(it.clone().count(), count);
        assert_eq!(it.size_hint(), (count, Some(count)));

        while let Some(_cap) = it.next() {
            count -= 1;
            assert_eq!(it.len(), count);
            assert_eq!(it.clone().count(), count);
            assert_eq!(it.size_hint(), (count, Some(count)));
        }

        assert_eq!(count, 0);

        assert_eq!(it.len(), 0);
        assert_eq!(it.clone().count(), 0);
        assert_eq!(it.size_hint(), (0, Some(0)));
    }

    #[test]
    fn test_cap_bits() {
        let mut mask: u64 = 0;

        for cap in Cap::iter() {
            let cap_bits = cap.to_single_bitfield();
            assert_eq!(2u64.pow(cap as u32), cap_bits);
            mask |= cap_bits;
        }

        assert_eq!(mask, CAP_BITMASK);
    }

    #[test]
    fn test_supported_caps() {
        let supported_caps = Cap::probe_supported();

        // Check that the binary search worked properly
        for cap in Cap::iter() {
            if supported_caps.has(cap) {
                assert!(cap.is_supported());
            } else {
                assert!(!cap.is_supported());
            }
        }
    }
}
