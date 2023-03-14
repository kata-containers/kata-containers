// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2020-2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Rafs filesystem metadata layout and data structures.

use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::{OsStr, OsString};
use std::io::Result;
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;

use fuse_backend_rs::abi::fuse_abi::ROOT_ID;
use nydus_utils::ByteSize;

use crate::metadata::layout::v5::RAFSV5_ALIGNMENT;

/// Version number for Rafs v4.
pub const RAFS_SUPER_VERSION_V4: u32 = 0x400;
/// Version number for Rafs v5.
pub const RAFS_SUPER_VERSION_V5: u32 = 0x500;
/// Version number for Rafs v6.
pub const RAFS_SUPER_VERSION_V6: u32 = 0x600;
/// Minimal version of Rafs supported.
pub const RAFS_SUPER_MIN_VERSION: u32 = RAFS_SUPER_VERSION_V4;
/// Inode number for Rafs root inode.
pub const RAFS_ROOT_INODE: u64 = ROOT_ID;

/// Type for filesystem xattr attribute key.
pub type XattrName = Vec<u8>;
/// Type for filesystem xattr attribute value.
pub type XattrValue = Vec<u8>;

pub mod v5;
pub mod v6;

pub enum RafsBlobTable {
    V5(v5::RafsV5BlobTable),
    V6(v6::RafsV6BlobTable),
}

#[doc(hidden)]
#[macro_export]
macro_rules! impl_bootstrap_converter {
    ($T: ty) => {
        impl TryFrom<&[u8]> for &$T {
            type Error = std::io::Error;

            fn try_from(buf: &[u8]) -> std::result::Result<Self, Self::Error> {
                let ptr = buf.as_ptr() as *const u8;
                if buf.len() != size_of::<$T>()
                    || ptr as usize & (std::mem::align_of::<$T>() - 1) != 0
                {
                    return Err(einval!("convert failed"));
                }

                Ok(unsafe { &*(ptr as *const $T) })
            }
        }

        impl TryFrom<&mut [u8]> for &mut $T {
            type Error = std::io::Error;

            fn try_from(buf: &mut [u8]) -> std::result::Result<Self, Self::Error> {
                let ptr = buf.as_ptr() as *mut u8 as *const u8;
                if buf.len() != size_of::<$T>()
                    || ptr as usize & (std::mem::align_of::<$T>() - 1) != 0
                {
                    return Err(einval!("convert failed"));
                }

                Ok(unsafe { &mut *(ptr as *const $T as *mut $T) })
            }
        }

        impl AsRef<[u8]> for $T {
            #[inline]
            fn as_ref(&self) -> &[u8] {
                let ptr = self as *const $T as *const u8;
                unsafe { std::slice::from_raw_parts(ptr, size_of::<$T>()) }
            }
        }

        impl AsMut<[u8]> for $T {
            #[inline]
            fn as_mut(&mut self) -> &mut [u8] {
                let ptr = self as *mut $T as *mut u8;
                unsafe { std::slice::from_raw_parts_mut(ptr, size_of::<$T>()) }
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! impl_pub_getter_setter {
    ($G: ident, $S: ident, $F: ident, $U: ty) => {
        #[inline]
        pub fn $G(&self) -> $U {
            <$U>::from_le(self.$F)
        }

        #[inline]
        pub fn $S(&mut self, $F: $U) {
            self.$F = <$U>::to_le($F);
        }
    };
}

/// Parse a utf8 byte slice into two strings.
pub fn parse_string(buf: &[u8]) -> Result<(&str, &str)> {
    std::str::from_utf8(buf)
        .map(|origin| {
            if let Some(pos) = origin.find('\0') {
                let (a, b) = origin.split_at(pos);
                (a, &b[1..])
            } else {
                (origin, "")
            }
        })
        .map_err(|e| einval!(format!("failed in parsing string, {:?}", e)))
}

/// Convert a byte slice into OsStr.
pub fn bytes_to_os_str(buf: &[u8]) -> &OsStr {
    OsStr::from_bytes(buf)
}

/// Parse a byte slice into xattr pairs and invoke the callback for each xattr pair.
///
/// The iteration breaks if the callback returns false.
pub fn parse_xattr<F>(data: &[u8], size: usize, mut cb: F) -> Result<()>
where
    F: FnMut(&OsStr, XattrValue) -> bool,
{
    if data.len() < size {
        return Err(einval!("invalid xattr content size"));
    }

    let mut rest_data = &data[0..size];
    let mut i: usize = 0;

    while i < size {
        if rest_data.len() < size_of::<u32>() {
            return Err(einval!(
                "invalid xattr content, no enough data for xattr pair size"
            ));
        }

        let (pair_size, rest) = rest_data.split_at(size_of::<u32>());
        let pair_size = u32::from_le_bytes(
            pair_size
                .try_into()
                .map_err(|_| einval!("failed to parse xattr pair size"))?,
        ) as usize;
        i += size_of::<u32>();

        if rest.len() < pair_size {
            return Err(einval!(
                "inconsistent xattr (size, data) pair, size is too big"
            ));
        }

        let (pair, rest) = rest.split_at(pair_size);
        if let Some(pos) = pair.iter().position(|&c| c == 0) {
            let (name, value) = pair.split_at(pos);
            let name = OsStr::from_bytes(name);
            let value = value[1..].to_vec();
            if !cb(name, value) {
                break;
            }
        }

        i += pair_size;
        rest_data = rest;
    }

    Ok(())
}

/// Parse a byte slice into xattr name list.
pub fn parse_xattr_names(data: &[u8], size: usize) -> Result<Vec<XattrName>> {
    let mut result = Vec::new();

    parse_xattr(data, size, |name, _| {
        result.push(name.as_bytes().to_vec());
        true
    })?;

    Ok(result)
}

/// Parse a 'buf' to xattr value by xattr name.
pub fn parse_xattr_value(data: &[u8], size: usize, name: &OsStr) -> Result<Option<XattrValue>> {
    let mut value = None;

    parse_xattr(data, size, |_name, _value| {
        if _name == name {
            value = Some(_value);
            // stop the iteration if we found the xattr name.
            return false;
        }
        true
    })?;

    Ok(value)
}

/// Rafs inode extended attributes.
///
/// An extended attribute is a (String, String) pair associated with a inode.
#[derive(Clone, Default)]
pub struct RafsXAttrs {
    pairs: HashMap<OsString, XattrValue>,
}

impl RafsXAttrs {
    /// Create a new instance of `RafsV5Xattrs`.
    pub fn new() -> Self {
        Self {
            pairs: HashMap::new(),
        }
    }

    /// Get size needed to store the extended attributes.
    pub fn size(&self) -> usize {
        let mut size: usize = 0;

        for (key, value) in self.pairs.iter() {
            size += size_of::<u32>();
            size += key.byte_size() + 1 + value.len();
        }

        size
    }

    /// Get extended attribute with  key `name`.
    pub fn get(&self, name: &OsStr) -> Option<&XattrValue> {
        self.pairs.get(name)
    }

    /// Add or update an extended attribute.
    pub fn add(&mut self, name: OsString, value: XattrValue) {
        self.pairs.insert(name, value);
    }

    /// Remove an extended attribute
    pub fn remove(&mut self, name: &OsStr) {
        self.pairs.remove(name);
    }

    /// Check whether there's any extended attribute.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

pub(crate) struct MetaRange {
    start: u64,
    size: u64,
}

impl MetaRange {
    pub fn new(start: u64, size: u64, aligned_size: bool) -> std::io::Result<Self> {
        let mask = RAFSV5_ALIGNMENT as u64 - 1;
        if start & mask == 0
            && (!aligned_size || size & mask == 0)
            && start.checked_add(size).is_some()
        {
            Ok(MetaRange { start, size })
        } else {
            Err(einval!(format!(
                "invalid metadata range {}:{}",
                start, size
            )))
        }
    }

    #[allow(dead_code)]
    pub fn start(&self) -> u64 {
        self.start
    }

    #[allow(dead_code)]
    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn end(&self) -> u64 {
        self.start + self.size
    }

    pub fn is_subrange_of(&self, other: &MetaRange) -> bool {
        self.start >= other.start && self.end() <= other.end()
    }

    pub fn intersect_with(&self, other: &MetaRange) -> bool {
        self.start < other.end() && self.end() > other.start
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;
    use std::ffi::OsString;
    use vm_memory::ByteValued;

    #[repr(transparent)]
    struct MockU32 {
        v: u32,
    }

    impl_bootstrap_converter!(MockU32);

    #[test]
    fn test_bootstrap_convert() {
        let mut value = 0x504030201u64;
        let buf = value.as_mut_slice();

        let v: std::io::Result<&MockU32> = (&buf[1..5]).try_into();
        assert!(v.is_err());

        let v: std::io::Result<&MockU32> = (&buf[0..3]).try_into();
        assert!(v.is_err());

        let v: std::io::Result<&mut MockU32> = (&mut buf[0..4]).try_into();
        let v = v.unwrap();
        assert_eq!(v.v, 0x4030201);
        assert_eq!(v.as_mut().len(), 4);
        assert_eq!(v.as_ref(), &[0x1u8, 0x2u8, 0x3u8, 0x4u8]);
    }

    #[test]
    fn test_parse_string() {
        let (str1, str2) = parse_string(&[b'a']).unwrap();
        assert_eq!(str1, "a");
        assert_eq!(str2, "");

        let (str1, str2) = parse_string(&[b'a', 0]).unwrap();
        assert_eq!(str1, "a");
        assert_eq!(str2, "");

        let (str1, str2) = parse_string(&[b'a', 0, b'b']).unwrap();
        assert_eq!(str1, "a");
        assert_eq!(str2, "b");

        let (str1, str2) = parse_string(&[b'a', 0, b'b', 0]).unwrap();
        assert_eq!(str1, "a");
        assert_eq!(str2, "b\0");

        parse_string(&[0xffu8, 0xffu8, 0xffu8, 0xffu8, 0xffu8]).unwrap_err();
    }

    #[test]
    fn test_parse_xattrs() {
        let buf = [0x4u8, 0x0, 0x0, 0x0, b'a', 0, b'b'];
        parse_xattr_names(&buf, 3).unwrap_err();
        parse_xattr_names(&buf, 8).unwrap_err();
        parse_xattr_names(&buf, 7).unwrap_err();

        let buf = [0x3u8, 0x0, 0x0, 0x0, b'a', 0, b'b'];
        let names = parse_xattr_names(&buf, 7).unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], &[b'a']);

        let value = parse_xattr_value(&buf, 7, &OsString::from("a")).unwrap();
        assert_eq!(value, Some(vec![b'b']));
    }

    #[test]
    fn test_meta_range() {
        assert!(MetaRange::new(u64::MAX, 1, true).is_err());
        assert!(MetaRange::new(u64::MAX, 1, true).is_err());
        assert!(MetaRange::new(1, 1, true).is_err());
        assert!(MetaRange::new(8, 0, true).is_ok());
        assert!(MetaRange::new(8, 1, true).is_err());
        assert_eq!(MetaRange::new(8, 8, true).unwrap().start(), 8);
        assert_eq!(MetaRange::new(8, 8, true).unwrap().size(), 8);
        assert_eq!(MetaRange::new(8, 8, true).unwrap().end(), 16);

        let range = MetaRange::new(16, 16, true).unwrap();

        assert!(!MetaRange::new(0, 8, true).unwrap().is_subrange_of(&range));
        assert!(!MetaRange::new(0, 16, true).unwrap().is_subrange_of(&range));
        assert!(!MetaRange::new(8, 8, true).unwrap().is_subrange_of(&range));
        assert!(!MetaRange::new(8, 16, true).unwrap().is_subrange_of(&range));
        assert!(!MetaRange::new(8, 24, true).unwrap().is_subrange_of(&range));
        assert!(MetaRange::new(16, 8, true).unwrap().is_subrange_of(&range));
        assert!(MetaRange::new(16, 16, true).unwrap().is_subrange_of(&range));
        assert!(MetaRange::new(24, 8, true).unwrap().is_subrange_of(&range));
        assert!(!MetaRange::new(24, 16, true).unwrap().is_subrange_of(&range));
        assert!(!MetaRange::new(32, 8, true).unwrap().is_subrange_of(&range));

        assert!(!MetaRange::new(0, 8, true).unwrap().intersect_with(&range));
        assert!(!MetaRange::new(0, 16, true).unwrap().intersect_with(&range));
        assert!(MetaRange::new(0, 24, true).unwrap().intersect_with(&range));
        assert!(MetaRange::new(8, 16, true).unwrap().intersect_with(&range));
        assert!(!MetaRange::new(8, 8, true).unwrap().intersect_with(&range));
        assert!(MetaRange::new(16, 8, true).unwrap().intersect_with(&range));
        assert!(MetaRange::new(16, 16, true).unwrap().intersect_with(&range));
        assert!(MetaRange::new(16, 24, true).unwrap().intersect_with(&range));
        assert!(MetaRange::new(24, 8, true).unwrap().intersect_with(&range));
        assert!(MetaRange::new(24, 16, true).unwrap().intersect_with(&range));
        assert!(!MetaRange::new(32, 8, true).unwrap().intersect_with(&range));
    }
}
