// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use std::fs;

const NON_PRIV_USER: &str = "nobody";

pub fn drop_privs() -> Result<()> {
    if nix::unistd::Uid::effective().is_root() {
        privdrop::PrivDrop::default()
            .chroot("/")
            .user(NON_PRIV_USER)
            .apply()
            .map_err(|e| anyhow!("Failed to drop privileges to user {}: {}", NON_PRIV_USER, e))?;
    }

    Ok(())
}

const PROC_VERSION_FILE: &str = "/proc/version";

pub fn get_kernel_version(proc_version_file: &str) -> Result<String> {
    let contents = fs::read_to_string(proc_version_file)
        .context(format!("Failed to read file {}", proc_version_file))?;

    let fields: Vec<&str> = contents.split_whitespace().collect();

    if fields.len() < 3 {
        return Err(anyhow!("unexpected contents in file {}", proc_version_file));
    }

    let kernel_version = String::from(fields[2]);
    Ok(kernel_version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_drop_privs() {
        let res = drop_privs();
        assert!(res.is_ok());
    }

    #[test]
    fn test_kernel_version_empty_input() {
        let res = get_kernel_version("").unwrap_err().to_string();
        let err_msg = format!("Failed to read file {}", "");
        assert_eq!(res, err_msg);
    }

    #[test]
    fn test_kernel_version_valid_input() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("proc-version");
        let path = file_path.clone();
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(
            file,
            "Linux version 5.15.0-75-generic (buildd@lcy02-amd64-045)"
        )
        .unwrap();
        let kernel = get_kernel_version(path.to_str().unwrap()).unwrap();
        assert_eq!(kernel, "5.15.0-75-generic");
    }

    #[test]
    fn test_kernel_version_system_input() {
        let res = get_kernel_version(PROC_VERSION_FILE);
        assert!(res.is_ok());
    }

    #[test]
    fn test_kernel_version_invalid_input() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("proc-version");
        let path = file_path.clone();
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(file, "Linux-version-5.15.0-75-generic").unwrap();
        let actual = get_kernel_version(path.to_str().unwrap())
            .unwrap_err()
            .to_string();
        let expected = format!("unexpected contents in file {}", path.to_str().unwrap());
        assert_eq!(actual, expected);
    }
}
