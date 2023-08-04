// Copyright 2023 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use std::fs::{read_to_string, OpenOptions};
use std::io::prelude::*;
use std::path::Path;

pub fn is_enabled() -> bool {
    match read_to_string("/sys/module/apparmor/parameters/enabled") {
        Ok(enabled) => enabled.starts_with('Y'),
        Err(_) => false,
    }
}

pub fn apply_apparmor(profile: &str) -> Result<()> {
    let exec_name = format!("exec {}", profile);

    let mut attr_path = Path::new("/proc/self/attr/apparmor/exec");
    if !attr_path.exists() {
        // Fall back to the old convention
        attr_path = Path::new("/proc/self/attr/exec");
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(attr_path)?;
    file.write_all(exec_name.as_str().as_bytes())
        .with_context(|| format!("failed to apply the AppArmor profile: {}", profile))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PROFILE: &str = "kata-test-profile";

    #[test]
    fn test_apply_apparmor() {
        let ret = apply_apparmor(TEST_PROFILE);
        if !is_enabled() {
            assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
        }
    }
}
