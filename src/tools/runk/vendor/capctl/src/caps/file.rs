use std::convert::TryInto;
use std::ffi::{CString, OsStr};
use std::fmt;
use std::io;
use std::os::unix::prelude::*;

use crate::sys;

use super::cap_text::{caps_from_text, caps_to_text, ParseCapsError};
use super::{CapSet, CapState};

/// Represents the capabilities attached to a file.
///
/// # `FromStr` and `Display` implementations
///
/// Like [`CapState`], this struct's implementations of  `FromStr` and `Display` use the same format
/// as `libcap`'s `cap_from_text()` and `cap_to_text()`. See `CapState`'s [documentation on
/// this](./struct.CapState.html#fromstr-and-display-implementations) for more details.
///
/// [`CapState`]: ./struct.CapState.html
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct FileCaps {
    /// The "effective" bit. If this is set on a file, then during an `execve()` the kernel will
    /// raise all the capabilities from the file's `permitted` set in the process's new effective
    /// capability set.
    pub effective: bool,
    /// The permitted capability set. These capabilities are automatically added to the process's
    /// new permitted capability set.
    pub permitted: CapSet,
    /// The inheritable capability set. Any of these capabilities that are also present in the
    /// process's inheritable capability set before an `execve()` are added to the new process's
    /// permitted capability set.
    pub inheritable: CapSet,
    /// The root user ID of the user namespace in which file capabilities were added to this file.
    /// See capabilities(7) for more details. This is only set to a non-`None` value for version 3
    /// file capabilities.
    pub rootid: Option<libc::uid_t>,
}

impl FileCaps {
    /// Construct an empty `FileCaps` object.
    #[inline]
    pub fn empty() -> Self {
        Self {
            effective: false,
            permitted: CapSet::empty(),
            inheritable: CapSet::empty(),
            rootid: None,
        }
    }

    /// Get the file capabilities attached to the file identified by `path`.
    ///
    /// If an error occurs while retrieving information on the capabilities from the given file,
    /// this method returns `Err(<error>)`. Otherwise, if the given file has no file capabilities
    /// attached, this method returns `Ok(None)`. Otherwise, this method returns
    /// `Ok(Some(<capabilities>))`.
    pub fn get_for_file<P: AsRef<OsStr>>(path: P) -> io::Result<Option<Self>> {
        let mut data = [0; sys::XATTR_CAPS_MAX_SIZE];

        let path = CString::new(path.as_ref().as_bytes())?;

        let ret = unsafe {
            libc::getxattr(
                path.as_ptr(),
                sys::XATTR_NAME_CAPS.as_ptr() as *const libc::c_char,
                data.as_mut_ptr() as *mut libc::c_void,
                data.len(),
            )
        };

        Self::extract_attr_or_error(&data, ret)
    }

    /// Get the file capabilities attached to the open file identified by the file descriptor `fd`.
    ///
    /// See [`get_for_file()`](#method.get_for_file) for more information.
    pub fn get_for_fd(fd: RawFd) -> io::Result<Option<Self>> {
        let mut data = [0; sys::XATTR_CAPS_MAX_SIZE];

        let ret = unsafe {
            libc::fgetxattr(
                fd,
                sys::XATTR_NAME_CAPS.as_ptr() as *const libc::c_char,
                data.as_mut_ptr() as *mut libc::c_void,
                data.len(),
            )
        };

        Self::extract_attr_or_error(&data, ret)
    }

    fn extract_attr_or_error(data: &[u8], attr_res: isize) -> io::Result<Option<Self>> {
        if attr_res >= 0 {
            Ok(Some(Self::unpack_attrs(&data[..(attr_res as usize)])?))
        } else {
            let err = io::Error::last_os_error();

            if err.raw_os_error() == Some(libc::ENODATA) {
                Ok(None)
            } else {
                Err(err)
            }
        }
    }

    /// From the raw data from the `security.capability` extended attribute of a file, construct a
    /// new `FileCaps` object representing the same data.
    ///
    /// Most users should call [`get_for_file()`] or [`get_for_fd()`]; those methods call this
    /// method internally.
    ///
    /// [`get_for_file()`]: #method.get_for_file
    /// [`get_for_fd()`]: #method.get_for_fd
    pub fn unpack_attrs(attrs: &[u8]) -> io::Result<Self> {
        let len = attrs.len();

        if len < 4 {
            return Err(io::Error::from_raw_os_error(libc::EINVAL));
        }

        let magic = u32::from_le_bytes(attrs[0..4].try_into().unwrap());
        let version = magic & sys::VFS_CAP_REVISION_MASK;
        let flags = magic & sys::VFS_CAP_FLAGS_MASK;

        let effective = (flags & sys::VFS_CAP_FLAGS_EFFECTIVE) != 0;

        match (version, len) {
            (sys::VFS_CAP_REVISION_2, sys::XATTR_CAPS_SZ_2) => Ok(FileCaps {
                effective,
                permitted: CapSet::from_bitmasks_u32(
                    u32::from_le_bytes(attrs[4..8].try_into().unwrap()),
                    u32::from_le_bytes(attrs[12..16].try_into().unwrap()),
                ),
                inheritable: CapSet::from_bitmasks_u32(
                    u32::from_le_bytes(attrs[8..12].try_into().unwrap()),
                    u32::from_le_bytes(attrs[16..20].try_into().unwrap()),
                ),
                rootid: None,
            }),

            (sys::VFS_CAP_REVISION_3, sys::XATTR_CAPS_SZ_3) => Ok(FileCaps {
                effective,
                permitted: CapSet::from_bitmasks_u32(
                    u32::from_le_bytes(attrs[4..8].try_into().unwrap()),
                    u32::from_le_bytes(attrs[12..16].try_into().unwrap()),
                ),
                inheritable: CapSet::from_bitmasks_u32(
                    u32::from_le_bytes(attrs[8..12].try_into().unwrap()),
                    u32::from_le_bytes(attrs[16..20].try_into().unwrap()),
                ),
                rootid: Some(u32::from_le_bytes(attrs[20..24].try_into().unwrap())),
            }),

            (sys::VFS_CAP_REVISION_1, sys::XATTR_CAPS_SZ_1) => Ok(FileCaps {
                effective,
                permitted: CapSet::from_bitmask_truncate(u32::from_le_bytes(
                    attrs[4..8].try_into().unwrap(),
                ) as u64),
                inheritable: CapSet::from_bitmask_truncate(u32::from_le_bytes(
                    attrs[8..12].try_into().unwrap(),
                ) as u64),
                rootid: None,
            }),

            (_, _) => Err(io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    /// Set the file capabilities attached to the file identified by `path` to the state
    /// represented by this object.
    #[inline]
    pub fn set_for_file<P: AsRef<OsStr>>(&self, path: P) -> io::Result<()> {
        let path = CString::new(path.as_ref().as_bytes())?;

        let mut buf = [0u8; sys::XATTR_CAPS_MAX_SIZE];
        let len = self.pack_into(&mut buf);

        debug_assert!(len <= buf.len());

        if unsafe {
            libc::setxattr(
                path.as_ptr(),
                sys::XATTR_NAME_CAPS.as_ptr() as *const libc::c_char,
                buf.as_ptr() as *const libc::c_void,
                len,
                0,
            )
        } < 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Set the file capabilities attached to the open file identified by the file descriptor `fd`
    /// to the state represented by this object.
    #[inline]
    pub fn set_for_fd(&self, fd: RawFd) -> io::Result<()> {
        let mut buf = [0u8; sys::XATTR_CAPS_MAX_SIZE];
        let len = self.pack_into(&mut buf);

        debug_assert!(len <= buf.len());

        if unsafe {
            libc::fsetxattr(
                fd,
                sys::XATTR_NAME_CAPS.as_ptr() as *const libc::c_char,
                buf.as_ptr() as *const libc::c_void,
                len,
                0,
            )
        } < 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn pack_into(&self, buf: &mut [u8]) -> usize {
        let mut magic = if self.rootid.is_some() {
            sys::VFS_CAP_REVISION_3
        } else {
            sys::VFS_CAP_REVISION_2
        };

        if self.effective {
            magic |= sys::VFS_CAP_FLAGS_EFFECTIVE;
        }

        let mut len = 20;

        buf[..4].copy_from_slice(&magic.to_le_bytes());
        buf[4..8].copy_from_slice(&(self.permitted.bits as u32).to_le_bytes());
        buf[8..12].copy_from_slice(&(self.inheritable.bits as u32).to_le_bytes());
        buf[12..16].copy_from_slice(&((self.permitted.bits >> 32) as u32).to_le_bytes());
        buf[16..20].copy_from_slice(&((self.inheritable.bits >> 32) as u32).to_le_bytes());

        if let Some(rootid) = self.rootid {
            buf[len..len + 4].copy_from_slice(&rootid.to_le_bytes());
            len += 4;
        }

        len
    }

    /// "Pack" the file capabilities represented by this object; i.e. convert it to the raw binary
    /// data as stored in the extended attribute.
    ///
    /// **Note**: Most users should call [`set_for_file()`] or [`set_for_fd()`]; those methods
    /// handle the details of "packing" file capabilities internally.
    ///
    /// This is the reverse of [`unpack_attrs()`]. As a result, "packing" the object using this
    /// method and then "unpacking" it using `unpack_attrs()` should always return a `FileCaps`
    /// object that represents the same state. So:
    ///
    /// ```
    /// # use capctl::caps::FileCaps;
    /// let fcaps = FileCaps::empty();
    /// assert_eq!(FileCaps::unpack_attrs(&fcaps.pack_attrs()).unwrap(), fcaps);
    /// ```
    ///
    /// (Note, however, that the reverse is not always true. For example, version 1 file
    /// capabilities can be "unpacked", but they will be "packed" as version 2 file capabilities,
    /// and as a result the binary data will be different.)
    ///
    /// [`set_for_file()`]: #method.set_for_file
    /// [`set_for_fd()`]: #method.set_for_fd
    /// [`unpack_attrs()`]: #method.unpack_attrs
    #[inline]
    pub fn pack_attrs(&self) -> Vec<u8> {
        let mut buf = vec![0u8; sys::XATTR_CAPS_MAX_SIZE];

        let len = self.pack_into(&mut buf);
        buf.truncate(len);

        buf
    }

    /// Remove the file capabilities attached to the file identified by `path`.
    #[inline]
    pub fn remove_for_file<P: AsRef<OsStr>>(path: P) -> io::Result<()> {
        let path = CString::new(path.as_ref().as_bytes())?;

        if unsafe {
            libc::removexattr(
                path.as_ptr(),
                sys::XATTR_NAME_CAPS.as_ptr() as *const libc::c_char,
            )
        } < 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Remove the file capabilities attached to the open file identified by `fd`.
    #[inline]
    pub fn remove_for_fd(fd: RawFd) -> io::Result<()> {
        if unsafe { libc::fremovexattr(fd, sys::XATTR_NAME_CAPS.as_ptr() as *const libc::c_char) }
            < 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for FileCaps {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        caps_to_text(
            CapState {
                effective: if self.effective {
                    self.permitted
                } else {
                    CapSet::empty()
                },
                permitted: self.permitted,
                inheritable: self.inheritable,
            },
            f,
        )
    }
}

impl core::str::FromStr for FileCaps {
    type Err = ParseFileCapsError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match caps_from_text(s) {
            Ok(state) => {
                if !state.effective.is_empty() && state.effective != state.permitted {
                    return Err(ParseFileCapsError(ParseCapsError::BadFileEffective));
                }

                Ok(Self {
                    effective: !state.effective.is_empty(),
                    permitted: state.permitted,
                    inheritable: state.inheritable,
                    rootid: None,
                })
            }
            Err(e) => Err(ParseFileCapsError(e)),
        }
    }
}

/// Represents an error when parsing a `FileCaps` object from a string.
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ParseFileCapsError(ParseCapsError);

impl fmt::Display for ParseFileCapsError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for ParseFileCapsError {}

#[cfg(test)]
mod tests {
    use core::iter::FromIterator;
    use core::str::FromStr;

    use crate::caps::Cap;
    use crate::capset;

    use super::*;

    #[test]
    fn test_filecaps_empty() {
        let empty_caps = FileCaps::empty();
        assert!(!empty_caps.effective);
        assert!(empty_caps.permitted.is_empty());
        assert!(empty_caps.inheritable.is_empty());
        assert!(empty_caps.rootid.is_none());
    }

    #[test]
    fn test_filecaps_get() {
        let current_exe = std::env::current_exe().unwrap();

        FileCaps::get_for_file(&current_exe).unwrap();

        let f = std::fs::File::open(&current_exe).unwrap();
        FileCaps::get_for_fd(f.as_raw_fd()).unwrap();

        assert_eq!(
            FileCaps::get_for_file(current_exe.join("sub"))
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ENOTDIR)
        );
        assert_eq!(
            FileCaps::get_for_fd(-1).unwrap_err().raw_os_error(),
            Some(libc::EBADF)
        );
    }

    #[test]
    fn test_filecaps_pack_unpack() {
        assert_eq!(
            FileCaps::unpack_attrs(b"").unwrap_err().raw_os_error(),
            Some(libc::EINVAL)
        );
        assert_eq!(
            FileCaps::unpack_attrs(b"\x00\x00\x00")
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EINVAL)
        );
        assert_eq!(
            FileCaps::unpack_attrs(b"\x00\x00\x00\x00")
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EINVAL)
        );

        // Version 1
        assert_eq!(
            FileCaps::unpack_attrs(b"\x00\x00\x00\x01\x01\x00\x00\x00\x01\x00\x00\x00").unwrap(),
            FileCaps {
                effective: false,
                permitted: CapSet::from_iter(vec![Cap::CHOWN]),
                inheritable: CapSet::from_iter(vec![Cap::CHOWN]),
                rootid: None,
            },
        );

        // Round-tripping Version 2 and Version 3 capabilities

        for (attr_data, fcaps) in [
            // Version 2 (real example, from Wireshark's /usr/bin/dumpcap)
            (
                b"\x01\x00\x00\x02\x020\x00\x00\x020\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00".as_ref(),
                 FileCaps {
                    effective: true,
                    permitted: CapSet::from_iter(vec![Cap::DAC_OVERRIDE, Cap::NET_ADMIN, Cap::NET_RAW]),
                    inheritable: CapSet::from_iter(vec![
                        Cap::DAC_OVERRIDE,
                        Cap::NET_ADMIN,
                        Cap::NET_RAW
                    ]),
                    rootid: None,
                }
            ),

            // Version 3
            (
                b"\x00\x00\x00\x03\x020\x00\x00\x020\x00\x00\x04\x00\x00\x00\x08\x00\x00\x00\xe8\x03\x00\x00".as_ref(),
                FileCaps {
                    effective: false,
                    permitted: CapSet::from_iter(vec![Cap::DAC_OVERRIDE, Cap::NET_ADMIN, Cap::NET_RAW, Cap::SYSLOG]),
                    inheritable: CapSet::from_iter(vec![Cap::DAC_OVERRIDE, Cap::NET_ADMIN, Cap::NET_RAW, Cap::WAKE_ALARM]),
                    rootid: Some(1000),
                }
            ),
        ].iter() {
            assert_eq!(FileCaps::unpack_attrs(attr_data).unwrap(), *fcaps);

            assert_eq!(&fcaps.pack_attrs(), attr_data);
        }
    }

    #[test]
    fn test_filecaps_set_error() {
        let current_exe = std::env::current_exe().unwrap();

        assert_eq!(
            FileCaps::empty()
                .set_for_file(current_exe.join("sub"))
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ENOTDIR)
        );
        assert_eq!(
            FileCaps::empty().set_for_fd(-1).unwrap_err().raw_os_error(),
            Some(libc::EBADF)
        );
    }

    #[test]
    fn test_filecaps_remove_error() {
        let current_exe = std::env::current_exe().unwrap();

        assert_eq!(
            FileCaps::remove_for_file(current_exe.join("sub"))
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ENOTDIR)
        );
        assert_eq!(
            FileCaps::remove_for_fd(-1).unwrap_err().raw_os_error(),
            Some(libc::EBADF)
        );
    }

    #[test]
    fn test_filecaps_parse() {
        // caps_from_text() has more extensive tests; we can be a little loose here

        assert_eq!(
            FileCaps::from_str("cap_chown=eip cap_chown-i cap_syslog+i").unwrap(),
            FileCaps {
                effective: true,
                permitted: capset!(Cap::CHOWN),
                inheritable: capset!(Cap::SYSLOG),
                rootid: None,
            }
        );

        assert_eq!(
            FileCaps::from_str("cap_chown=p").unwrap(),
            FileCaps {
                effective: false,
                permitted: capset!(Cap::CHOWN),
                inheritable: capset!(),
                rootid: None,
            }
        );

        assert_eq!(
            FileCaps::from_str("cap_chown=e").unwrap_err().to_string(),
            "Effective set must be either empty or same as permitted set",
        );

        assert_eq!(
            FileCaps::from_str("cap_noexist+p").unwrap_err().to_string(),
            "Unknown capability"
        );
    }

    #[test]
    fn test_filecaps_display() {
        for fcaps in [
            FileCaps::empty(),
            FileCaps {
                effective: true,
                permitted: capset!(Cap::CHOWN),
                inheritable: capset!(Cap::SYSLOG),
                rootid: None,
            },
            FileCaps {
                effective: false,
                permitted: capset!(Cap::CHOWN),
                inheritable: capset!(),
                rootid: None,
            },
            FileCaps {
                effective: false,
                permitted: !capset!(Cap::CHOWN),
                inheritable: capset!(Cap::CHOWN),
                rootid: None,
            },
            FileCaps {
                effective: false,
                permitted: !capset!(Cap::CHOWN),
                inheritable: !capset!(Cap::CHOWN),
                rootid: None,
            },
        ]
        .iter()
        {
            let s = fcaps.to_string();

            assert_eq!(s.parse::<FileCaps>().unwrap(), *fcaps, "{:?}", s);
        }
    }
}
