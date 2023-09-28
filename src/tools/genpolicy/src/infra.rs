// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::pod;
use crate::policy;
use crate::volume;

use anyhow::Result;
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::path::Path;
use std::str;

const INFRA_MOUNT_DESTINATIONS: [&'static str; 8] = [
    "/sys/fs/cgroup",
    "/etc/hosts",
    "/dev/termination-log",
    "/etc/hostname",
    "/etc/resolv.conf",
    "/dev/shm",
    "/var/run/secrets/kubernetes.io/serviceaccount",
    "/var/run/secrets/azure/tokens",
];

const PAUSE_CONTAINER_ANNOTATIONS: [(&'static str, &'static str); 6] = [
    ("io.kubernetes.cri.container-type", "sandbox"),
    ("io.kubernetes.cri.sandbox-id", "^[a-z0-9]{64}$"),
    ("io.kubernetes.cri.sandbox-log-directory", "^/var/log/pods/$(sandbox-namespace)_$(sandbox-name)_[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"),
    ("io.katacontainers.pkg.oci.container_type", "pod_sandbox"),
    ("io.kubernetes.cri.sandbox-namespace", "default"),
    ("io.katacontainers.pkg.oci.bundle_path", "/run/containerd/io.containerd.runtime.v2.task/k8s.io/$(bundle-id)")
];

const OTHER_CONTAINERS_ANNOTATIONS: [(&'static str, &'static str); 4] = [
    (
        "io.katacontainers.pkg.oci.bundle_path",
        "/run/containerd/io.containerd.runtime.v2.task/k8s.io/$(bundle-id)",
    ),
    ("io.kubernetes.cri.sandbox-id", "^[a-z0-9]{64}$"),
    ("io.katacontainers.pkg.oci.container_type", "pod_container"),
    ("io.kubernetes.cri.container-type", "container"),
];

#[derive(Debug, Serialize, Deserialize)]
pub struct InfraPolicy {
    pub pause_container: policy::KataSpec,
    pub other_container: policy::KataSpec,
    pub volumes: Volumes,
    kata_config: KataConfig,
    pub request_defaults: policy::RequestDefaults,
    pub common: policy::CommonData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volumes {
    pub emptyDir: EmptyDirVolume,
    pub emptyDir_memory: EmptyDirVolume,
    pub configMap: ConfigMapVolume,
    pub confidential_configMap: ConfigMapVolume,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyDirVolume {
    pub mount_type: String,
    pub mount_source: String,
    pub mount_point: String,
    pub driver: String,
    pub fstype: String,
    pub options: Vec<String>,

    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigMapVolume {
    pub mount_type: String,
    pub mount_source: String,
    pub mount_point: String,
    pub driver: String,
    pub fstype: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KataConfig {
    pub confidential_guest: bool,
}

impl InfraPolicy {
    pub fn new(infra_data_file: &str) -> Result<Self> {
        debug!("Loading containers policy data...");

        if let Ok(file) = File::open(infra_data_file) {
            let mut infra_policy: Self = serde_json::from_reader(file).unwrap();
            debug!("infra_policy = {:?}", &infra_policy);

            add_pause_container_data(&mut infra_policy.pause_container);
            add_other_container_data(&mut infra_policy.other_container);

            debug!("Finished loading containers policy data.");
            Ok(infra_policy)
        } else {
            panic!("Cannot open file {}. Please copy it to the current directory or specify the path to it using the -i parameter.", 
                infra_data_file);
        }
    }
}

// Change process fields based on K8s infrastructure rules.
pub fn get_process(process: &mut policy::KataProcess, infra_policy: &policy::KataSpec) {
    if let Some(infra_process) = &infra_policy.Process {
        if process.User.UID == 0 {
            process.User.UID = infra_process.User.UID;
        }
        if process.User.GID == 0 {
            process.User.GID = infra_process.User.GID;
        }

        process.User.AdditionalGids = infra_process.User.AdditionalGids.to_vec();
        process.User.Username = String::from(&infra_process.User.Username);
        add_missing_strings(&infra_process.Args, &mut process.Args);

        add_missing_strings(&infra_process.Env, &mut process.Env);
    }
}

impl InfraPolicy {
    pub fn get_policy_mounts(
        &self,
        policy_mounts: &mut Vec<policy::KataMount>,
        infra_mounts: &Vec<policy::KataMount>,
        yaml_container: &pod::Container,
        is_pause_container: bool,
    ) {
        let mut rootfs_access = "rw".to_string();
        if yaml_container.read_only_root_filesystem() {
            rootfs_access = "ro".to_string();
        }

        for infra_mount in infra_mounts {
            if keep_infra_mount(&infra_mount, &yaml_container.volumeMounts) {
                let mut mount = infra_mount.clone();

                if mount.source.is_empty() && mount.type_.eq("bind") {
                    if let Some(file_name) = Path::new(&mount.destination).file_name() {
                        if let Some(file_name) = file_name.to_str() {
                            mount.source = "$(sfprefix)".to_string();
                            mount.source += file_name;
                            mount.source += "$";
                        }
                    }
                }

                if let Some(policy_mount) = policy_mounts
                    .iter_mut()
                    .find(|m| m.destination.eq(&infra_mount.destination))
                {
                    // Update an already existing mount.
                    policy_mount.type_ = String::from(&mount.type_);
                    policy_mount.source = String::from(&mount.source);
                    policy_mount.options = mount.options.iter().map(String::from).collect();
                } else {
                    // Add a new mount.
                    if !is_pause_container {
                        if infra_mount.destination.eq("/etc/hostname")
                            || infra_mount.destination.eq("/etc/resolv.conf")
                        {
                            mount.options.push(rootfs_access.to_string());
                        }
                    }

                    policy_mounts.push(mount);
                }
            }
        }
    }
}

fn keep_infra_mount(
    infra_mount: &policy::KataMount,
    yaml_mounts: &Option<Vec<pod::VolumeMount>>,
) -> bool {
    if INFRA_MOUNT_DESTINATIONS
        .iter()
        .any(|&i| i == infra_mount.destination)
    {
        return true;
    }

    if let Some(mounts) = yaml_mounts {
        for mount in mounts {
            if mount.mountPath.eq(&infra_mount.destination) {
                return true;
            }
        }
    }

    false
}

pub fn add_annotations(
    annotations: &mut BTreeMap<String, String>,
    infra_policy: &policy::KataSpec,
) {
    if let Some(infra_annotations) = &infra_policy.Annotations {
        for annotation in infra_annotations {
            annotations
                .entry(annotation.0.to_string())
                .or_insert(annotation.1.clone());
        }
    }
}

pub fn get_linux(linux: &mut policy::KataLinux, infra_linux: &Option<policy::KataLinux>) {
    if let Some(infra) = infra_linux {
        if !infra.MaskedPaths.is_empty() {
            linux.MaskedPaths = infra.MaskedPaths.clone();
        }
        if !infra.ReadonlyPaths.is_empty() {
            linux.ReadonlyPaths = infra.ReadonlyPaths.clone();
        }
    }
}

fn add_missing_strings(src: &Vec<String>, dest: &mut Vec<String>) {
    for src_string in src {
        if !dest.contains(src_string) {
            dest.push(src_string.clone());
        }
    }
    debug!("src = {:?}, dest = {:?}", src, dest)
}

fn add_pause_container_data(oci: &mut policy::KataSpec) {
    if let Some(process) = &mut oci.Process {
        process.Args = vec!["/pause".to_string()];
    }

    for annotation in PAUSE_CONTAINER_ANNOTATIONS {
        if let Some(annotations) = &mut oci.Annotations {
            annotations
                .entry(annotation.0.to_string())
                .or_insert(annotation.1.to_string());
        } else {
            let mut annotations = BTreeMap::new();
            annotations.insert(annotation.0.to_string(), annotation.1.to_string());
            oci.Annotations = Some(annotations);
        }
    }

    if oci.Linux.is_none() {
        oci.Linux = Some(Default::default());
    }
    if let Some(linux) = &mut oci.Linux {
        linux.MaskedPaths = vec![
            "/proc/acpi".to_string(),
            "/proc/asound".to_string(),
            "/proc/kcore".to_string(),
            "/proc/keys".to_string(),
            "/proc/latency_stats".to_string(),
            "/proc/timer_list".to_string(),
            "/proc/timer_stats".to_string(),
            "/proc/sched_debug".to_string(),
            "/sys/firmware".to_string(),
            "/proc/scsi".to_string(),
        ];
        linux.ReadonlyPaths = vec![
            "/proc/bus".to_string(),
            "/proc/fs".to_string(),
            "/proc/irq".to_string(),
            "/proc/sys".to_string(),
            "/proc/sysrq-trigger".to_string(),
        ];
    }
}

fn add_other_container_data(oci: &mut policy::KataSpec) {
    for annotation in OTHER_CONTAINERS_ANNOTATIONS {
        if let Some(annotations) = &mut oci.Annotations {
            annotations
                .entry(annotation.0.to_string())
                .or_insert(annotation.1.to_string());
        } else {
            let mut annotations = BTreeMap::new();
            annotations.insert(annotation.0.to_string(), annotation.1.to_string());
            oci.Annotations = Some(annotations);
        }
    }
}

impl InfraPolicy {
    pub fn get_mount_and_storage(
        &self,
        policy_mounts: &mut Vec<policy::KataMount>,
        storages: &mut Vec<policy::SerializedStorage>,
        yaml_volume: &volume::Volume,
        yaml_mount: &pod::VolumeMount,
    ) {
        if let Some(emptyDir) = &yaml_volume.emptyDir {
            let memory_medium = if let Some(medium) = &emptyDir.medium {
                medium == "Memory"
            } else {
                false
            };
            Self::empty_dir_mount_and_storage(&self.volumes, policy_mounts, storages, yaml_mount, memory_medium);
        } else if yaml_volume.persistentVolumeClaim.is_some() || yaml_volume.azureFile.is_some() {
            self.shared_bind_mount(yaml_mount, policy_mounts, "rprivate", "rw");
        } else if yaml_volume.hostPath.is_some() {
            self.host_path_mount(yaml_mount, yaml_volume, policy_mounts);
        } else if yaml_volume.configMap.is_some() || yaml_volume.secret.is_some() {
            Self::config_map_mount_and_storage(
                &self.volumes,
                policy_mounts,
                storages,
                yaml_mount,
                self.kata_config.confidential_guest,
            );
        } else if yaml_volume.projected.is_some() {
            self.shared_bind_mount(yaml_mount, policy_mounts, "rprivate", "ro");
        } else if yaml_volume.downwardAPI.is_some() {
            self.downward_api_mount(yaml_mount, policy_mounts);
        } else {
            todo!("Unsupported volume type {:?}", yaml_volume);
        }
    }

    fn empty_dir_mount_and_storage(
        infra_volumes: &Volumes,
        policy_mounts: &mut Vec<policy::KataMount>,
        storages: &mut Vec<policy::SerializedStorage>,
        yaml_mount: &pod::VolumeMount,
        memory_medium: bool,
    ) {
        let infra_empty_dir = if memory_medium {
            &infra_volumes.emptyDir_memory
        } else {
            &infra_volumes.emptyDir
        };
        debug!("Infra emptyDir: {:?}", infra_empty_dir);

        if yaml_mount.subPathExpr.is_none() {
            storages.push(policy::SerializedStorage {
                driver: infra_empty_dir.driver.clone(),
                driver_options: Vec::new(),
                source: infra_empty_dir.source.clone(),
                fstype: infra_empty_dir.fstype.clone(),
                options: infra_empty_dir.options.clone(),
                mount_point: infra_empty_dir.mount_point.clone() + &yaml_mount.name + "$",
                fs_group: None,
            });
        }

        let source = if yaml_mount.subPathExpr.is_some() {
            let file_name = Path::new(&yaml_mount.mountPath).file_name().unwrap();
            let name = OsString::from(file_name).into_string().unwrap();
            infra_volumes.configMap.mount_source.clone() + &name + "$"
        } else {
            infra_empty_dir.mount_source.to_string() + &yaml_mount.name + "$"
        };

        let type_ = if yaml_mount.subPathExpr.is_some() {
            "bind".to_string()
        } else {
            infra_empty_dir.mount_type.clone()
        };

        policy_mounts.push(policy::KataMount {
            destination: yaml_mount.mountPath.to_string(),
            type_,
            source,
            options: vec![
                "rbind".to_string(),
                "rprivate".to_string(),
                "rw".to_string(),
            ],
        });
    }

    fn shared_bind_mount(
        &self,
        yaml_mount: &pod::VolumeMount,
        policy_mounts: &mut Vec<policy::KataMount>,
        propagation: &str,
        access: &str,
    ) {
        let mut source = "$(sfprefix)".to_string();
        if let Some(byte_index) = str::rfind(&yaml_mount.mountPath, '/') {
            source += str::from_utf8(&yaml_mount.mountPath.as_bytes()[byte_index + 1..]).unwrap();
        } else {
            source += &yaml_mount.mountPath;
        }
        source += "$";

        let destination = yaml_mount.mountPath.to_string();
        let type_ = "bind".to_string();
        let options = vec![
            "rbind".to_string(),
            propagation.to_string(),
            access.to_string(),
        ];

        if let Some(policy_mount) = policy_mounts
            .iter_mut()
            .find(|m| m.destination.eq(&destination))
        {
            debug!(
                "shared_bind_mount: updating destination = {}, source = {}",
                &destination, &source
            );
            policy_mount.type_ = type_;
            policy_mount.source = source;
            policy_mount.options = options;
        } else {
            debug!(
                "shared_bind_mount: adding destination = {}, source = {}",
                &destination, &source
            );
            policy_mounts.push(policy::KataMount {
                destination,
                type_,
                source,
                options,
            });
        }
    }

    fn host_path_mount(
        &self,
        yaml_mount: &pod::VolumeMount,
        yaml_volume: &volume::Volume,
        policy_mounts: &mut Vec<policy::KataMount>,
    ) {
        let host_path = yaml_volume.hostPath.as_ref().unwrap().path.clone();
        let path = Path::new(&host_path);

        let mut biderectional = false;
        if let Some(mount_propagation) = &yaml_mount.mountPropagation {
            if mount_propagation.eq("Bidirectional") {
                debug!("host_path_mount: Bidirectional");
                biderectional = true;
            }
        }

        // TODO:
        //
        // - When volume.hostPath.path: /dev/ttyS0
        //      "source": "/dev/ttyS0"
        // - When volume.hostPath.path: /tmp/results
        //      "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-results$"
        //
        // What is the reason for this source path difference in the Guest OS?
        if !path.starts_with("/dev/") && !path.starts_with("/sys/") {
            debug!("host_path_mount: calling shared_bind_mount");
            let propagation = if biderectional { "rshared" } else { "rprivate" };
            self.shared_bind_mount(yaml_mount, policy_mounts, propagation, "rw");
        } else {
            let dest = yaml_mount.mountPath.to_string();
            let type_ = "bind".to_string();
            let mount_option = if biderectional { "rshared" } else { "rprivate" };
            let options = vec![
                "rbind".to_string(),
                mount_option.to_string(),
                "rw".to_string(),
            ];

            if let Some(policy_mount) = policy_mounts.iter_mut().find(|m| m.destination.eq(&dest)) {
                debug!(
                    "host_path_mount: updating destination = {}, source = {}",
                    &dest, &host_path
                );
                policy_mount.type_ = type_;
                policy_mount.source = host_path;
                policy_mount.options = options;
            } else {
                debug!(
                    "host_path_mount: adding destination = {}, source = {}",
                    &dest, &host_path
                );
                policy_mounts.push(policy::KataMount {
                    destination: dest,
                    type_,
                    source: host_path,
                    options,
                });
            }
        }
    }

    fn config_map_mount_and_storage(
        infra_volumes: &Volumes,
        policy_mounts: &mut Vec<policy::KataMount>,
        storages: &mut Vec<policy::SerializedStorage>,
        yaml_mount: &pod::VolumeMount,
        confidential_guest: bool,
    ) {
        let infra_config_map = if confidential_guest {
            &infra_volumes.confidential_configMap
        } else {
            &infra_volumes.configMap
        };

        debug!(
            "config_map_mount_and_storage: infra configMap: {:?}",
            infra_config_map
        );

        if !confidential_guest {
            let mount_path = Path::new(&yaml_mount.mountPath).file_name().unwrap();
            let mount_path_str = OsString::from(mount_path).into_string().unwrap();

            storages.push(policy::SerializedStorage {
                driver: infra_config_map.driver.clone(),
                driver_options: Vec::new(),
                source: infra_config_map.mount_source.clone() + &yaml_mount.name + "$",
                fstype: infra_config_map.fstype.clone(),
                options: infra_config_map.options.clone(),
                mount_point: infra_config_map.mount_point.clone() + &mount_path_str + "$",
                fs_group: None,
            });
        }

        let file_name = Path::new(&yaml_mount.mountPath).file_name().unwrap();
        let name = OsString::from(file_name).into_string().unwrap();
        policy_mounts.push(policy::KataMount {
            destination: yaml_mount.mountPath.to_string(),
            type_: infra_config_map.mount_type.to_string(),
            source: infra_config_map.mount_point.clone() + &name + "$",
            options: infra_config_map.options.clone(),
        });
    }

    fn downward_api_mount(
        &self,
        yaml_mount: &pod::VolumeMount,
        policy_mounts: &mut Vec<policy::KataMount>,
    ) {
        let mut source = "$(sfprefix)".to_string();
        if let Some(byte_index) = str::rfind(&yaml_mount.mountPath, '/') {
            source += str::from_utf8(&yaml_mount.mountPath.as_bytes()[byte_index + 1..]).unwrap();
        } else {
            source += &yaml_mount.mountPath;
        }
        source += "$";

        let destination = yaml_mount.mountPath.to_string();
        let type_ = "bind".to_string();
        let mount_option = "rprivate".to_string();
        let options = vec!["rbind".to_string(), mount_option, "ro".to_string()];

        if let Some(policy_mount) = policy_mounts
            .iter_mut()
            .find(|m| m.destination.eq(&destination))
        {
            debug!(
                "downward_api_mount: updating destination = {}, source = {}",
                &destination, &source
            );
            policy_mount.type_ = type_;
            policy_mount.source = source;
            policy_mount.options = options;
        } else {
            debug!(
                "downward_api_mount: adding destination = {}, source = {}",
                &destination, &source
            );
            policy_mounts.push(policy::KataMount {
                destination,
                type_,
                source,
                options,
            });
        }
    }
}
