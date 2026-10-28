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
    /// Network namespace the runtime created for the sandbox and has to remove.
    #[serde(default)]
    pub created_netns: Option<String>,
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
        assert_eq!(state.created_netns, None);
    }

    // Every field in state.json and what recovery does with it. A field that
    // nothing reads is noise in the one file recovery trusts, and a field
    // recovery relies on must not silently disappear.
    const SANDBOX_FIELDS: &[(&str, &str)] = &[
        (
            "sandbox_type",
            "RuntimeHandlerManager::cleanup() picks the runtime",
        ),
        ("resource", "ResourceManager::restore()"),
        ("hypervisor", "VirtSandbox::restore()"),
        (
            "vmm_exit_confirmed",
            "confirm_restored_vmm_exit() skips the kill",
        ),
        ("config_path", "cleanup_config() reloads the configuration"),
        ("created_netns", "VirtSandbox::cleanup() removes it"),
    ];
    const RESOURCE_FIELDS: &[(&str, &str)] = &[
        ("endpoint", "rebind_saved_physical_endpoints()"),
        ("cgroup_state", "CgroupsResource::restore(), then deleted"),
        ("ephemeral_disks", "cleanup_ephemeral_disks()"),
    ];
    const HYPERVISOR_FIELDS: &[(&str, &str)] = &[
        (
            "hypervisor_type",
            "VirtSandbox::restore() picks the hypervisor",
        ),
        ("id", "QEMU cleanup builds its VM directory from it"),
        (
            "vm_path",
            "CH, Dragonball and Firecracker cleanup remove it",
        ),
        (
            "jailed",
            "Dragonball and Firecracker cleanup unmount the jail",
        ),
        ("cached_block_devices", "Dragonball cleanup unmounts them"),
        ("config", "the rootless VMM user and vm_cleanup()"),
        (
            "jailer_root",
            "Dragonball cleanup unmounts the jail under it",
        ),
    ];

    fn keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    fn names(fields: &[(&str, &str)]) -> Vec<String> {
        let mut names: Vec<_> = fields.iter().map(|(name, _)| name.to_string()).collect();
        names.sort();
        names
    }

    #[test]
    fn every_saved_field_is_one_recovery_uses() {
        let mut state: SandboxState = serde_json::from_str(LEGACY_STATE).unwrap();
        state.config_path = Some(PathBuf::from(
            "/opt/kata/share/defaults/kata-containers/configuration-qemu.toml",
        ));
        state.created_netns = Some("/run/netns/cnitest".to_string());
        let json = serde_json::to_value(&state).unwrap();

        assert_eq!(keys(&json), names(SANDBOX_FIELDS));
        assert_eq!(keys(&json["resource"]), names(RESOURCE_FIELDS));
        assert_eq!(keys(&json["hypervisor"]), names(HYPERVISOR_FIELDS));

        let reloaded: SandboxState = serde_json::from_value(json).unwrap();
        assert_eq!(reloaded.config_path, state.config_path);
        assert_eq!(reloaded.created_netns, state.created_netns);
    }

    #[test]
    fn a_state_saved_early_loads_with_what_it_has() {
        let state: SandboxState = serde_json::from_str(
            r#"{"sandbox_type": "virt_container", "hypervisor": {"hypervisor_type": "qemu"}}"#,
        )
        .unwrap();
        assert!(state.resource.is_none());
        let h = state.hypervisor.unwrap();
        assert!(h.id.is_empty());
        assert!(h.config.security_info.rootless_user.is_none());
    }
}
