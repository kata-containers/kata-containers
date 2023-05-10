// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::anyhow;
use anyhow::Result;
use cgroups;
use cgroups::freezer::{FreezerController, FreezerState};
use std::{thread, time};

// Try to remove the provided cgroups path five times with increasing delay between tries.
// If after all there are not removed cgroups, an appropriate error will be returned.
pub fn remove_cgroup_dir(cgroup: &cgroups::Cgroup) -> Result<()> {
    let mut retries = 5;
    let mut delay = time::Duration::from_millis(10);
    while retries != 0 {
        if retries != 5 {
            delay *= 2;
            thread::sleep(delay);
        }

        if cgroup.delete().is_ok() {
            return Ok(());
        }

        retries -= 1;
    }

    Err(anyhow!("failed to remove cgroups paths"))
}

// Make sure we get a stable freezer state, so retry if the cgroup is still undergoing freezing.
pub fn get_freezer_state(freezer: &FreezerController) -> Result<FreezerState> {
    let mut retries = 10;
    while retries != 0 {
        let state = freezer.state()?;
        match state {
            FreezerState::Thawed => return Ok(FreezerState::Thawed),
            FreezerState::Frozen => return Ok(FreezerState::Frozen),
            FreezerState::Freezing => {
                // sleep for 10 ms, wait for the cgroup to finish freezing
                thread::sleep(time::Duration::from_millis(10));
                retries -= 1;
            }
        }
    }
    Ok(FreezerState::Freezing)
}

// check whether freezer state is frozen
pub fn is_paused(cgroup: &cgroups::Cgroup) -> Result<bool> {
    let freezer_controller: &FreezerController = cgroup
        .controller_of()
        .ok_or_else(|| anyhow!("failed to get freezer controller"))?;
    let freezer_state = get_freezer_state(freezer_controller)?;
    match freezer_state {
        FreezerState::Frozen => Ok(true),
        _ => Ok(false),
    }
}

pub fn freeze(cgroup: &cgroups::Cgroup, state: FreezerState) -> Result<()> {
    let freezer_controller: &FreezerController = cgroup
        .controller_of()
        .ok_or_else(|| anyhow!("failed to get freezer controller"))?;
    match state {
        FreezerState::Frozen => {
            freezer_controller.freeze()?;
        }
        FreezerState::Thawed => {
            freezer_controller.thaw()?;
        }
        _ => return Err(anyhow!("invalid freezer state")),
    }
    Ok(())
}
