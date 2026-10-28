// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

use hypervisor::hypervisor_persist::HypervisorState;
use resource::resource_persist::ResourceState;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SandboxState {
    pub sandbox_type: String,
    pub resource: Option<ResourceState>,
    pub hypervisor: Option<HypervisorState>,
    #[serde(default)]
    pub vmm_exit_confirmed: bool,
    /// Configuration file the sandbox was created with. The shim `delete`
    /// subcommand gets no shimv2 options to find it otherwise.
    #[serde(default)]
    pub config_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Written before the dead hypervisor fields were dropped. A shim upgraded
    // while sandboxes run must still clean up the ones it did not create.
    const LEGACY_STATE: &str = include_str!("../testdata/legacy-state.json");

    #[test]
    fn state_written_by_an_older_shim_still_loads() {
        let state: SandboxState = serde_json::from_str(LEGACY_STATE).unwrap();

        let h = state.hypervisor.unwrap();
        assert_eq!(h.hypervisor_type, "qemu");
        assert_eq!(h.id, "0123456789abcdef");
        let user = h.config.security_info.rootless_user.unwrap();
        assert_eq!((user.uid, user.user_name.as_str()), (1001, "kata-6461"));

        let r = state.resource.unwrap();
        assert_eq!(
            r.cgroup_state.unwrap().path.as_deref(),
            Some("/kubepods/besteffort/pod1/0123456789abcdef")
        );
        assert_eq!(r.endpoint.len(), 1);
        assert_eq!(state.config_path, None);
    }
}
