// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ffi::OsString;
use std::io::Result;
use std::path::{Path, PathBuf};

use crate::eother;

/// Get bundle path (current working directory).
pub fn get_bundle_path() -> Result<PathBuf> {
    std::env::current_dir()
}

/// Get the basename of the canonicalized path
pub fn get_base_name<P: AsRef<Path>>(src: P) -> Result<OsString> {
    let s = src.as_ref().canonicalize()?;
    s.file_name().map(|v| v.to_os_string()).ok_or_else(|| {
        eother!(
            "failed to get base name of path {}",
            src.as_ref().to_string_lossy()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_base_name() {
        assert_eq!(&get_base_name("/etc/hostname").unwrap(), "hostname");
        assert_eq!(&get_base_name("/bin").unwrap(), "bin");
        assert!(&get_base_name("/").is_err());
        assert!(&get_base_name("").is_err());
        assert!(get_base_name("/no/such/path________yeah").is_err());
    }
}
