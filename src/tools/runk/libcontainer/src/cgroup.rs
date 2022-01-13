// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use rustjail::cgroups::fs::Manager as CgroupManager;
use std::{
    path::Path,
    {fs, thread, time},
};

pub fn destroy_cgroup(cgroup_mg: &CgroupManager) -> Result<()> {
    for path in cgroup_mg.paths.values() {
        remove_cgroup_dir(Path::new(path))?;
    }

    Ok(())
}

// Try to remove the provided cgroups path five times with increasing delay between tries.
// If after all there are not removed cgroups, an appropriate error will be returned.
fn remove_cgroup_dir(path: &Path) -> Result<()> {
    let mut retries = 5;
    let mut delay = time::Duration::from_millis(10);
    while retries != 0 {
        if retries != 5 {
            delay *= 2;
            thread::sleep(delay);
        }

        if !path.exists() || fs::remove_dir(path).is_ok() {
            return Ok(());
        }

        retries -= 1;
    }

    return Err(anyhow!("failed to remove cgroups paths: {:?}", path));
}
