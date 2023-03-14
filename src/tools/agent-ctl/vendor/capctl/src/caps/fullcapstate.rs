use std::fs;
use std::io;
use std::io::prelude::*;

use super::{ambient, bounding, CapSet, CapState};

/// Represents the "full" capability state of a thread (i.e. the contents of all 5 capability
/// sets and some additional information).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct FullCapState {
    pub permitted: CapSet,
    pub effective: CapSet,
    pub inheritable: CapSet,
    pub ambient: CapSet,
    pub bounding: CapSet,
    pub no_new_privs: bool,
}

impl FullCapState {
    /// Construct an empty `FullCapState` object.
    #[inline]
    pub fn empty() -> Self {
        Self {
            permitted: CapSet::empty(),
            effective: CapSet::empty(),
            inheritable: CapSet::empty(),
            ambient: CapSet::empty(),
            bounding: CapSet::empty(),
            no_new_privs: false,
        }
    }

    /// Get the full capability state of the current thread.
    ///
    /// This is equivalent to `FullCapState::get_for_pid(0)`. However, this method uses the kernel
    /// APIs to retrieve information instead of examining files in `/proc`.
    pub fn get_current() -> io::Result<Self> {
        let state = CapState::get_current()?;

        Ok(Self {
            permitted: state.permitted,
            effective: state.effective,
            inheritable: state.inheritable,
            ambient: ambient::probe().unwrap_or_default(),
            bounding: bounding::probe(),
            no_new_privs: crate::prctl::get_no_new_privs()?,
        })
    }

    /// Get the full capability state of the process (or thread) with the given PID (or TID) by
    /// examining special files in `/proc`.
    ///
    /// If `pid` is 0, this method gets the capability state of the current thread.
    pub fn get_for_pid(pid: libc::pid_t) -> io::Result<Self> {
        let file_res = match pid.cmp(&0) {
            core::cmp::Ordering::Less => return Err(io::Error::from_raw_os_error(libc::EINVAL)),
            core::cmp::Ordering::Equal => fs::File::open("/proc/thread-self/status"),
            core::cmp::Ordering::Greater => fs::File::open(format!("/proc/{}/status", pid)),
        };

        let f = match file_res {
            Ok(f) => f,
            Err(e) if e.raw_os_error() == Some(libc::ENOENT) => {
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
            Err(e) => return Err(e),
        };

        let mut reader = io::BufReader::new(f);
        let mut line = String::new();

        let mut res = Self::empty();

        while reader.read_line(&mut line)? > 0 {
            if line.ends_with('\n') {
                line.pop();
            }

            if let Some(i) = line.find(":\t") {
                let value = &line[i + 2..];

                let set = match &line[..i] {
                    "CapPrm" => Some(&mut res.permitted),
                    "CapEff" => Some(&mut res.effective),
                    "CapInh" => Some(&mut res.inheritable),
                    "CapBnd" => Some(&mut res.bounding),
                    "CapAmb" => Some(&mut res.ambient),
                    "NoNewPrivs" => {
                        res.no_new_privs = value == "1";
                        None
                    }
                    _ => None,
                };

                if let Some(set) = set {
                    match u64::from_str_radix(value, 16) {
                        Ok(bitmask) => *set = CapSet::from_bitmask_truncate(bitmask),
                        Err(e) => {
                            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
                        }
                    }
                }
            }

            line.clear();
        }

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_proc() {
        assert_eq!(
            FullCapState::get_current().unwrap(),
            FullCapState::get_for_pid(0).unwrap(),
        );

        assert_eq!(
            FullCapState::get_current().unwrap(),
            FullCapState::get_for_pid(unsafe { libc::syscall(libc::SYS_gettid) } as libc::pid_t)
                .unwrap(),
        );
    }

    #[test]
    fn test_get_invalid_pid() {
        assert_eq!(
            FullCapState::get_for_pid(-1).unwrap_err().raw_os_error(),
            Some(libc::EINVAL)
        );

        assert_eq!(
            FullCapState::get_for_pid(libc::pid_t::MAX)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ESRCH)
        );
    }

    #[test]
    fn test_pid_1_match() {
        let state = CapState::get_for_pid(1).unwrap();
        let fullstate = FullCapState::get_for_pid(1).unwrap();

        assert_eq!(state.effective, fullstate.effective);
        assert_eq!(state.permitted, fullstate.permitted);
        assert_eq!(state.inheritable, fullstate.inheritable);
    }
}
