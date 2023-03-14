// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

pub trait ByteSize {
    fn byte_size(&self) -> usize;
}

impl ByteSize for OsString {
    fn byte_size(&self) -> usize {
        self.as_bytes().len()
    }
}

impl ByteSize for OsStr {
    fn byte_size(&self) -> usize {
        self.as_bytes().len()
    }
}

impl ByteSize for PathBuf {
    fn byte_size(&self) -> usize {
        self.as_os_str().byte_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_string_empty() {
        let os_str = OsStr::new("");
        let os_string = OsString::from("");

        assert_eq!(os_str.len(), 0);
        assert_eq!(os_str.byte_size(), 0);
        assert_eq!(os_string.len(), 0);
        assert_eq!(os_string.byte_size(), 0);
    }

    #[test]
    fn test_os_string_size() {
        let os_str = OsStr::new("foo");
        let os_string = OsString::from("foo");

        assert_eq!(os_str.len(), 3);
        assert_eq!(os_str.byte_size(), 3);
        assert_eq!(os_string.len(), 3);
        assert_eq!(os_string.byte_size(), 3);
    }

    #[test]
    fn test_pathbuf_size() {
        let mut path = PathBuf::new();

        assert_eq!(path.byte_size(), 0);

        path.push("/");
        assert_eq!(path.byte_size(), 1);

        path.push("test");
        assert_eq!(path.byte_size(), 5);

        // "/test/a"
        path.push("a");
        assert_eq!(path.byte_size(), 7);
    }
}
