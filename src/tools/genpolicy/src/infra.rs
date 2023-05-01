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
use std::ffi::OsString;
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfraPolicy {
    pub pause_container: policy::OciSpec,
    pub other_container: policy::OciSpec,
    pub volumes: Option<Volumes>,
    shared_files: SharedFiles,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Volumes {
    pub emptyDir: EmptyDirVolume,
    pub configMap: ConfigMapVolume,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct ConfigMapVolume {
    pub mount_type: String,
    pub mount_source: String,
    pub mount_point: String,
    pub driver: String,
    pub fstype: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
                Self::empty_dir_mount_and_storage(
                    &infra_volumes,
                    yaml_mount,
                    policy_mounts,
                    storages,
                );
            } else if yaml_volume.persistentVolumeClaim.is_some() {
                self.volume_claim_mount(yaml_mount, policy_mounts)?;
            } else if yaml_volume.hostPath.is_some() {
                self.host_path_mount(yaml_mount, policy_mounts)?;
            } else if yaml_volume.configMap.is_some() {
                Self::config_map_mount_and_storage(
                    &infra_volumes,
                    policy_mounts,
                    storages,
                    yaml_volume,
                    yaml_mount,
                )?;
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
    fn empty_dir_mount_and_storage(
        infra_volumes: &Volumes,
        yaml_mount: &yaml::VolumeMount,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
    ) {
        let infra_empty_dir = &infra_volumes.emptyDir;
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
    // containers:
    //   - image: "docker.io/library/busybox:1.36.0"
    //     name: busybox
    //     volumeMounts:
    //       - mountPath: /cm2
    //         name: cm2-volume
    // volumes:
    //   - name: cm2-volume
    //     configMap:
    //       name: config-map2
    //       items:
    //         - key: file1.json
    //           path: my-keys
    //
    // Corresponding output policy data:
    //
    // {
    //     "destination": "/cm2",
    //     "type": "bind",
    //     "source": "^/run/kata-containers/shared/containers/watchable/$(bundle-id)-[a-z0-9]{16}-cm2$",
    //     "options": [
    //       "rbind",
    //       "rprivate",
    //       "ro"
    //     ]
    // }
    //...
    // "storages": [
    //     {
    //       "driver": "watchable-bind",
    //       "driver_options": [],
    //       "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-cm2-volume$",
    //       "fstype": "bind",
    //       "options": [
    //         "rbind",
    //         "rprivate",
    //         "ro"
    //       ],
    //       "mount_point": "^/run/kata-containers/shared/containers/watchable/$(bundle-id)-[a-z0-9]{16}-cm2-volume$",
    //       "fs_group": {
    //         "group_id": 0,
    //         "group_change_policy": 0
    //       }
    //     }
    //  ]
    fn config_map_mount_and_storage(
        infra_volumes: &Volumes,
        policy_mounts: &mut Vec<oci::Mount>,
        storages: &mut Vec<policy::SerializedStorage>,
        _yaml_volume: &yaml::Volume,
        yaml_mount: &yaml::VolumeMount,
    ) -> Result<()> {
        let infra_config_map = &infra_volumes.configMap;
        info!("Infra configMap: {:?}", infra_config_map);

        storages.push(policy::SerializedStorage {
            driver: infra_config_map.driver.clone(),
            driver_options: Vec::new(),
            source: infra_config_map.mount_source.clone() + &yaml_mount.name + "$",
            fstype: infra_config_map.fstype.clone(),
            options: infra_config_map.options.clone(),
            mount_point: infra_config_map.mount_point.clone() + &yaml_mount.name + "$",
            fs_group: policy::SerializedFsGroup {
                group_id: 0,
                group_change_policy: 0,
            },
        });

        if let Some(file_name) = Path::new(&yaml_mount.mountPath).file_name() {
            if let Ok(name) = OsString::from(file_name).into_string() {
                policy_mounts.push(oci::Mount {
                    destination: yaml_mount.mountPath.to_string(),
                    r#type: infra_config_map.mount_type.to_string(),
                    source: infra_config_map.mount_point.clone() + &name + "$",
                    options: infra_config_map.options.clone(),
                });
            } else {
                panic!("Unsupported mount path: {:?}", &yaml_mount.mountPath);
            }
        } else {
            panic!("No file name in mount path: {:?}", &yaml_mount.mountPath);
        }

        Ok(())
    }
}
