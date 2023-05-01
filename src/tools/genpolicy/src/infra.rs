// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::policy;
use crate::yaml;

use anyhow::Result;
use log::info;
use oci;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::str;

const INFRA_MOUNT_DESTINATIONS: [&'static str; 7] = [
    "/sys/fs/cgroup",
    "/etc/hosts",
    "/dev/termination-log",
    "/etc/hostname",
    "/etc/resolv.conf",
    "/dev/shm",
    "/var/run/secrets/kubernetes.io/serviceaccount",
];

const PAUSE_CONTAINER_ANNOTATIONS: [(&'static str, &'static str); 7] = [
    ("io.kubernetes.cri.container-type", "sandbox"),
    ("io.kubernetes.cri.sandbox-id", "^[a-z0-9]{64}$"),
    ("nerdctl/network-namespace", "^/var/run/netns/cni-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"),
    ("io.kubernetes.cri.sandbox-log-directory", "^/var/log/pods/default_$(sandbox-name)_[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"),
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

// Attempting to validate IP addresses and/or port numbers related
// to the Kubernetes and any other services would be futile because
// the Guest VM's Host controls how these addresses and ports are
// implemented. For example, if the Guest VM tries to connect to
// some service listening on port 5000, the Host virtualization
// stack can redirect that connect request to a roque service listening
// on port 6000. To ensure that a container connects to the service
// that it expects, rather than some rogue service, is to implement
// mutually-authenticated TLS - or a similar protocol - between these
// two communication peers. The port and/or IP address values don't
// help with authenticating these peers and/or with the confidentiality
// of their communication.
/*
const OTHER_CONTAINERS_ENV: [&'static str; 8] = [
    "KUBERNETES_PORT_443_TCP_PROTO=tcp",
    "KUBERNETES_PORT_443_TCP_PORT=443",
    "KUBERNETES_PORT_443_TCP_ADDR=10.0.0.1",
    "KUBERNETES_SERVICE_HOST=10.0.0.1",
    "KUBERNETES_SERVICE_PORT=443",
    "KUBERNETES_SERVICE_PORT_HTTPS=443",
    "KUBERNETES_PORT=tcp://10.0.0.1:443",
    "KUBERNETES_PORT_443_TCP=tcp://10.0.0.1:443",
];
*/

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct InfraPolicy {
    pub pause_container: policy::OciSpec,
    pub other_container: policy::OciSpec,
    pub volumes: Option<Volumes>,
    shared_files: SharedFiles,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Volumes {
    pub emptyDir: Option<EmptyDirVolume>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EmptyDirVolume {
    pub mount_type: String,
    pub mount_point: String,
    pub mount_source: String,
    pub driver: String,
    pub source: String,
    pub fstype: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SharedFiles {
    source_path: String,
}

impl InfraPolicy {
    pub fn new(infra_data_file: &str) -> Result<Self> {
        info!("Loading containers policy data...");
        let mut infra_policy: Self = serde_json::from_reader(File::open(infra_data_file)?)?;
        add_pause_container_data(&mut infra_policy.pause_container);
        add_other_container_data(&mut infra_policy.other_container);
        info!("Finished loading containers policy data.");
        Ok(infra_policy)
    }
}

// Change process fields based on K8s infrastructure rules.
pub fn get_process(process: &mut policy::OciProcess, infra_policy: &policy::OciSpec) -> Result<()> {
    if let Some(infra_process) = &infra_policy.process {
        if process.user.uid == 0 {
            process.user.uid = infra_process.user.uid;
        }
        if process.user.gid == 0 {
            process.user.gid = infra_process.user.gid;
        }

        process.user.additional_gids = infra_process.user.additional_gids.to_vec();
        process.user.username = String::from(&infra_process.user.username);
        add_missing_strings(&infra_process.args, &mut process.args);

        add_missing_strings(&infra_process.env, &mut process.env);
    }

    Ok(())
}

impl InfraPolicy {
    pub fn get_policy_mounts(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        infra_mounts: &Vec<oci::Mount>,
        yaml_container: &yaml::Container,
        is_pause_container: bool,
    ) -> Result<()> {
        let mut rootfs_access = "rw".to_string();
        if yaml_container.read_only_root_filesystem() {
            rootfs_access = "ro".to_string();
        }

        for infra_mount in infra_mounts {
            if keep_infra_mount(&infra_mount, &yaml_container.volumeMounts) {
                let mut mount = infra_mount.clone();

                if mount.source.is_empty() && mount.r#type.eq("bind") {
                    if let Some(file_name) = Path::new(&mount.destination).file_name() {
                        if let Some(file_name) = file_name.to_str() {
                            mount.source += &self.shared_files.source_path;
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
                    policy_mount.r#type = String::from(&mount.r#type);
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
        Ok(())
    }
}

fn keep_infra_mount(
    infra_mount: &oci::Mount,
    yaml_mounts: &Option<Vec<yaml::VolumeMount>>,
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

pub fn get_annotations(
    annotations: &mut BTreeMap<String, String>,
    infra_policy: &policy::OciSpec,
) -> Result<()> {
    for annotation in &infra_policy.annotations {
        annotations.insert(annotation.0.clone(), annotation.1.clone());
    }

    Ok(())
}

pub fn get_linux(linux: &mut oci::Linux, infra_linux: &Option<oci::Linux>) -> Result<()> {
    if let Some(infra) = infra_linux {
        if !infra.masked_paths.is_empty() {
            linux.masked_paths = infra.masked_paths.clone();
        }
        if !infra.readonly_paths.is_empty() {
            linux.readonly_paths = infra.readonly_paths.clone();
        }
    }

    Ok(())
}

fn add_missing_strings(src: &Vec<String>, dest: &mut Vec<String>) {
    for src_string in src {
        if !dest.contains(src_string) {
            dest.push(src_string.clone());
        }
    }
    info!("src = {:?}, dest = {:?}", src, dest)
}

fn add_pause_container_data(oci: &mut policy::OciSpec) {
    if oci.process.is_none() {
        oci.process = Some(Default::default());
    }
    if let Some(process) = &mut oci.process {
        process.args = vec!["/pause".to_string()];
    }

    for annotation in PAUSE_CONTAINER_ANNOTATIONS {
        oci.annotations
            .entry(annotation.0.to_string())
            .or_insert(annotation.1.to_string());
    }

    if oci.linux.is_none() {
        oci.linux = Some(Default::default());
    }
    if let Some(linux) = &mut oci.linux {
        linux.masked_paths = vec![
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
        linux.readonly_paths = vec![
            "/proc/bus".to_string(),
            "/proc/fs".to_string(),
            "/proc/irq".to_string(),
            "/proc/sys".to_string(),
            "/proc/sysrq-trigger".to_string(),
        ];
    }
}

fn add_other_container_data(oci: &mut policy::OciSpec) {
    if oci.process.is_none() {
        oci.process = Some(Default::default());
    }

    /*
    if let Some(process) = &mut oci.process {
        for env_var in OTHER_CONTAINERS_ENV {
            let env_var_str = env_var.to_string();
            if !process.env.contains(&env_var_str) {
                process.env.push(env_var_str);
            }
        }
    }
    */

    for annotation in OTHER_CONTAINERS_ANNOTATIONS {
        oci.annotations
            .entry(annotation.0.to_string())
            .or_insert(annotation.1.to_string());
    }
}

impl InfraPolicy {
    pub fn get_mount_and_storage(
        &self,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        yaml_volume: &yaml::Volume,
        yaml_mount: &yaml::VolumeMount,
    ) -> Result<()> {
        if let Some(infra_volumes) = &self.volumes {
            if yaml_volume.emptyDir.is_some() {
                Self::mount_and_storage_empty_dir(
                    &infra_volumes,
                    &yaml_mount,
                    policy_mounts,
                    storages,
                );
            } else if yaml_volume.persistentVolumeClaim.is_some() {
                self.volume_claim_mount(&yaml_mount, policy_mounts)?;
            } else if yaml_volume.hostPath.is_some() {
                self.host_path_mount(&yaml_mount, policy_mounts)?;
            } else if yaml_volume.configMap.is_some() {
                self.config_map_mount(&yaml_mount, policy_mounts)?;
            } else {
                todo!("Unsupported volume type {:?}", yaml_volume);
            }
        }

        Ok(())
    }

    // Example of input yaml:
    //
    // containers:
    // - image: docker.io/library/busybox:1.36.0
    //   name: busybox
    //   volumeMounts:
    //   - mountPath: /busy1
    //     name: data
    // ...
    // volumes:
    // - name: data
    //   emptyDir: {}
    // ...
    //
    // Corresponding output policy data:
    //
    // {
    //    "destination": "/busy1",
    //    "type": "local",
    //    "source": "^/run/kata-containers/shared/containers/$(sandbox-id)/local/data$",
    //    "options": [
    //          "rbind",
    //          "rprivate",
    //          "rw"
    //     ]
    // }
    // ...
    // "storages": [
    //  {
    //      "driver": "local",
    //      "driver_options": [],
    //      "source": "local",
    //      "fstype": "local",
    //      "options": [
    //          "mode=0777"
    //      ],
    //      "mount_point": "/run/kata-containers/shared/containers/$(sandbox-id)/local/data",
    //      "fs_group": {
    //          "group_id": 0,
    //          "group_change_policy": 0
    //      }
    //  }
    // ]
    fn mount_and_storage_empty_dir(
        infra_volumes: &Volumes,
        yaml_mount: &yaml::VolumeMount,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
    ) {
        if let Some(infra_empty_dir) = &infra_volumes.emptyDir {
            info!("Infra emptyDir: {:?}", infra_empty_dir);
            let mut mount_source = infra_empty_dir.mount_source.to_string();
            mount_source += &yaml_mount.name;

            storages.push(policy::SerializedStorage {
                driver: infra_empty_dir.driver.clone(),
                driver_options: Vec::new(),
                source: infra_empty_dir.source.clone(),
                fstype: infra_empty_dir.fstype.clone(),
                options: infra_empty_dir.options.clone(),
                mount_point: infra_empty_dir.mount_point.clone() + &yaml_mount.name + "$",
                fs_group: policy::SerializedFsGroup {
                    group_id: 0,
                    group_change_policy: 0,
                },
            });

            mount_source += "$";

            policy_mounts.push(oci::Mount {
                destination: yaml_mount.mountPath.to_string(),
                r#type: infra_empty_dir.mount_type.to_string(),
                source: mount_source,
                options: vec![
                    "rbind".to_string(),
                    "rprivate".to_string(),
                    "rw".to_string(),
                ],
            });
        }
    }

    // Example of input yaml:
    //
    // containers:
    // - image: docker.io/library/busybox:1.36.0
    //   name: busybox
    //   volumeMounts:
    //   - mountPath: /my-volume
    //     name: my-pod-volume
    // ...
    // volumes:
    // - name: my-pod-volume
    //   persistentVolumeClaim:
    //   claimName: my-volume-claim
    // ...
    //
    // Corresponding output policy data:
    //
    // {
    //    "destination": "/my-volume",
    //    "type": "bind",
    //    "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-my-volume$",
    //    "options": [
    //          "rbind",
    //          "rprivate",
    //          "rw"
    //    ]
    // }
    fn volume_claim_mount(
        &self,
        yaml_mount: &yaml::VolumeMount,
        policy_mounts: &mut Vec<oci::Mount>,
    ) -> Result<()> {
        let mut mount_source = self.shared_files.source_path.to_string();

        if let Some(byte_index) = str::rfind(&yaml_mount.mountPath, '/') {
            mount_source += str::from_utf8(&yaml_mount.mountPath.as_bytes()[byte_index + 1..])?;
        } else {
            mount_source += &yaml_mount.mountPath;
        }

        mount_source += "$";

        policy_mounts.push(oci::Mount {
            destination: yaml_mount.mountPath.to_string(),
            r#type: "bind".to_string(),
            source: mount_source,
            options: vec![
                "rbind".to_string(),
                "rprivate".to_string(),
                "rw".to_string(),
            ],
        });

        Ok(())
    }

    // Example of input yaml:
    //
    // containers:
    // - image: docker.io/library/busybox:1.36.0
    //   name: busybox
    //   volumeMounts:
    //    - mountPath: /dev/ttyS0
    //      name: dev-ttys0
    // ...
    // volumes:
    //   - name: dev-ttys0
    //     hostPath:
    //       path: /dev/ttyS0
    // ...
    //
    // Corresponding output policy data:
    //
    // {
    //     "destination": "/dev/ttyS0",
    //     "type": "bind",
    //     "source": "/dev/ttyS0",
    //     "options": [
    //         "rbind",
    //         "rprivate",
    //         "rw"
    //     ]
    // }
    fn host_path_mount(
        &self,
        yaml_mount: &yaml::VolumeMount,
        policy_mounts: &mut Vec<oci::Mount>,
    ) -> Result<()> {
        policy_mounts.push(oci::Mount {
            destination: yaml_mount.mountPath.to_string(),
            r#type: "bind".to_string(),
            source: yaml_mount.mountPath.to_string(),
            options: vec![
                "rbind".to_string(),
                "rprivate".to_string(),
                "rw".to_string(),
            ],
        });

        Ok(())
    }

    // Example of input yaml:
    //
    // TBD
    fn config_map_mount(
        &self,
        _yaml_mount: &yaml::VolumeMount,
        _policy_mounts: &mut Vec<oci::Mount>,
    ) -> Result<()> {
        // TODO
        Ok(())
    }
}
