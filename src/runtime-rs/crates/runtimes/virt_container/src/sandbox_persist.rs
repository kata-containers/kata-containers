// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};

use common::SandboxNetworkEnv;
use hypervisor::hypervisor_persist::HypervisorState;
use kata_types::config::Hypervisor as HypervisorConfig;
use resource::cgroups::cgroup_persist::CgroupState;
use resource::resource_persist::ResourceState;
use serde::{Deserialize, Serialize};

use crate::VIRTCONTAINER;

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
    /// The state was saved before the sandbox existed. States from shims
    /// that did not write the field marked that by having no resources.
    #[serde(default)]
    pub bootstrap: bool,
    /// The runtime's keep_abnormal when the sandbox saved, so the decision
    /// to leave an abnormal sandbox alone does not hang on the external
    /// configuration file still saying what it said then. A state saved
    /// before the configuration was read carries none and falls back to
    /// the file.
    #[serde(default)]
    pub keep_abnormal: Option<bool>,
}

impl SandboxState {
    /// The state saved before the sandbox exists, for what the shim makes on
    /// the host while setting it up: the network namespace, the rootless
    /// VMM user, and the cgroups and swap directory the resource manager
    /// will create before the sandbox saves for itself.
    pub fn bootstrap(
        network_env: &SandboxNetworkEnv,
        config_path: Option<&Path>,
        hypervisor: Option<(&str, &HypervisorConfig)>,
        cgroup_state: Option<CgroupState>,
    ) -> Self {
        // The namespace is recorded before any network is constructed in
        // it: a death mid-construction leaves TAPs there that only the
        // recorded path lets the cleanup sweep.
        let netns_path = network_env.netns.clone();
        Self {
            sandbox_type: VIRTCONTAINER.to_string(),
            resource: match (cgroup_state, netns_path) {
                (None, None) => None,
                (cgroup_state, netns_path) => Some(ResourceState {
                    cgroup_state,
                    netns_path,
                    ..Default::default()
                }),
            },
            hypervisor: hypervisor.map(|(name, config)| HypervisorState {
                hypervisor_type: name.to_string(),
                config: config.clone(),
                ..Default::default()
            }),
            vmm_exit_confirmed: false,
            config_path: config_path.map(Path::to_path_buf),
            created_netns: network_env.created_netns(),
            bootstrap: true,
            keep_abnormal: None,
        }
    }

    /// Every state the sandbox saves for itself has its resources, so one
    /// without them was saved before the sandbox existed, even by a shim
    /// from before the explicit marker.
    pub fn is_bootstrap(&self) -> bool {
        self.bootstrap || self.resource.is_none()
    }
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

        // An old state recorded no keep_abnormal; the decision stays with
        // the configuration file for it.
        assert_eq!(state.keep_abnormal, None);

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
        ("bootstrap", "is_bootstrap() picks the cleanup path"),
        (
            "keep_abnormal",
            "cleanup() leaves an abnormal sandbox alone",
        ),
    ];
    const RESOURCE_FIELDS: &[(&str, &str)] = &[
        ("endpoint", "RestoredNetwork::remove() rebinds the VFs"),
        (
            "netns_path",
            "remove_kata_links() sweeps it, in recovery and bootstrap cleanup",
        ),
        ("cgroup_state", "CgroupsResource::restore(), then deleted"),
        ("ephemeral_disks", "cleanup_ephemeral_disks()"),
    ];
    const ENDPOINT_FIELDS: &[(&str, &str)] = &[(
        "physical_endpoint",
        "rebind_saved_physical_endpoints() gives the NIC back",
    )];
    const PHYSICAL_ENDPOINT_FIELDS: &[(&str, &str)] = &[
        ("bdf", "the device the rebind acts on"),
        ("driver", "the host driver it binds back"),
    ];
    const CGROUP_FIELDS: &[(&str, &str)] = &[
        ("path", "the sandbox cgroup to destroy"),
        ("overhead_path", "the overhead cgroup to destroy"),
        (
            "sandbox_cgroup_only",
            "whether an overhead cgroup exists at all",
        ),
    ];
    const DISK_FIELDS: &[(&str, &str)] = &[
        ("disk_path", "cleanup_ephemeral_disks() removes the disk"),
        (
            "source_path",
            "cleanup_ephemeral_disks() removes the direct-volume record",
        ),
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
        // The legacy fixture predates disk records; add one so its keys are
        // pinned like the other nested records'.
        state.resource.as_mut().unwrap().ephemeral_disks.push(
            resource::volume::EphemeralDiskInfo {
                disk_path: PathBuf::from("/run/kata-containers/0123/ephemeral/disk.img"),
                source_path: "/var/lib/kubelet/pods/p1/volumes/empty-dir/data".to_string(),
            },
        );
        let json = serde_json::to_value(&state).unwrap();

        assert_eq!(keys(&json), names(SANDBOX_FIELDS));
        assert_eq!(keys(&json["resource"]), names(RESOURCE_FIELDS));
        assert_eq!(keys(&json["hypervisor"]), names(HYPERVISOR_FIELDS));
        let endpoint = &json["resource"]["endpoint"][0];
        assert_eq!(keys(endpoint), names(ENDPOINT_FIELDS));
        assert_eq!(
            keys(&endpoint["physical_endpoint"]),
            names(PHYSICAL_ENDPOINT_FIELDS)
        );
        assert_eq!(
            keys(&json["resource"]["cgroup_state"]),
            names(CGROUP_FIELDS)
        );
        assert_eq!(
            keys(&json["resource"]["ephemeral_disks"][0]),
            names(DISK_FIELDS)
        );

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
        assert!(state.is_bootstrap());
        let h = state.hypervisor.unwrap();
        assert!(h.id.is_empty());
        assert!(h.config.security_info.rootless_user.is_none());
    }

    #[test]
    fn the_setup_state_records_what_the_shim_made() {
        let network_env = SandboxNetworkEnv {
            netns: Some("/run/netns/cni-1".to_string()),
            network_created: true,
        };
        let mut config = HypervisorConfig::default();
        config.security_info.rootless_user = Some(kata_types::config::hypervisor::RootlessUser {
            uid: 1001,
            user_name: "kata-6461".to_string(),
            ..Default::default()
        });
        let path = Path::new("/opt/kata/configuration.toml");

        let cgroups = CgroupState {
            path: Some("kubepods/besteffort/pod1/cri-0123".to_string()),
            ..Default::default()
        };
        let state =
            SandboxState::bootstrap(&network_env, Some(path), Some(("qemu", &config)), None);
        let state: SandboxState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert!(state.is_bootstrap());
        // The namespace is recorded before anything is constructed in it,
        // so a mid-construction death leaves a sweepable record.
        assert_eq!(
            state.resource.as_ref().unwrap().netns_path.as_deref(),
            Some("/run/netns/cni-1")
        );
        assert_eq!(state.created_netns.as_deref(), Some("/run/netns/cni-1"));
        assert_eq!(state.config_path.as_deref(), Some(path));
        let h = state.hypervisor.unwrap();
        assert_eq!(h.hypervisor_type, "qemu");
        let user = h.config.security_info.rootless_user.unwrap();
        assert_eq!((user.uid, user.user_name.as_str()), (1001, "kata-6461"));

        // A record that names the cgroups ahead of their creation is still a
        // bootstrap state, even though it has a resource section.
        let state = SandboxState::bootstrap(
            &network_env,
            Some(path),
            Some(("qemu", &config)),
            Some(cgroups),
        );
        let state: SandboxState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert!(state.is_bootstrap());
        assert_eq!(
            state
                .resource
                .unwrap()
                .cgroup_state
                .unwrap()
                .path
                .as_deref(),
            Some("kubepods/besteffort/pod1/cri-0123")
        );

        // A caller-provided namespace is not recorded as created, but it is
        // recorded for the sweep: the caller's namespace outlives a dead
        // sandbox, and so would the TAPs built in it.
        let given = SandboxNetworkEnv {
            network_created: false,
            ..network_env
        };
        let state = SandboxState::bootstrap(&given, None, None, None);
        assert_eq!(state.created_netns, None);
        assert_eq!(
            state.resource.unwrap().netns_path.as_deref(),
            Some("/run/netns/cni-1")
        );

        // No namespace, no cgroups: nothing for a resource section to say.
        let none = SandboxNetworkEnv {
            netns: None,
            network_created: false,
        };
        assert!(SandboxState::bootstrap(&none, None, None, None)
            .resource
            .is_none());
    }
}
