// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::pod;
use crate::policy;
use crate::settings;
use crate::volume;

use log::debug;
use protocols::agent;
use std::ffi::OsString;
use std::path::Path;
use std::str;

pub fn get_policy_mounts(
    settings: &settings::Settings,
    p_mounts: &mut Vec<policy::KataMount>,
    yaml_container: &pod::Container,
    is_pause_container: bool,
) {
    if let Some(volumeMounts) = &yaml_container.volumeMounts {
        for volumeMount in volumeMounts {
            if volumeMount.subPath.is_some() {
                panic!("Kata Containers doesn't support volumeMounts.subPath - see https://github.com/kata-containers/runtime/issues/2812");
            }
        }
    }

    let c_settings = settings.get_container_settings(is_pause_container);
    let settings_mounts = &c_settings.Mounts;
    let rootfs_access = if yaml_container.read_only_root_filesystem() {
        "ro"
    } else {
        "rw"
    };

    for s_mount in settings_mounts {
        if keep_settings_mount(settings, s_mount, &yaml_container.volumeMounts) {
            let mut mount = s_mount.clone();
            adjust_termination_path(&mut mount, yaml_container);

            if mount.source.is_empty() && mount.type_.eq("bind") {
                if let Some(file_name) = Path::new(&mount.destination).file_name() {
                    if let Some(file_name) = file_name.to_str() {
                        mount.source = format!("$(sfprefix){file_name}$");
                    }
                }
            }

            if let Some(policy_mount) = p_mounts
                .iter_mut()
                .find(|m| m.destination.eq(&s_mount.destination))
            {
                // Update an already existing mount.
                policy_mount.type_.clone_from(&mount.type_);
                policy_mount.source.clone_from(&mount.source);
                policy_mount.options = mount.options.iter().map(String::from).collect();
            } else {
                // Add a new mount.
                if !is_pause_container
                    && (s_mount.destination.eq("/etc/hostname")
                        || s_mount.destination.eq("/etc/resolv.conf"))
                {
                    mount.options.push(rootfs_access.to_string());
                }
                p_mounts.push(mount);
            }
        }
    }
}

fn keep_settings_mount(
    settings: &settings::Settings,
    s_mount: &policy::KataMount,
    yaml_mounts: &Option<Vec<pod::VolumeMount>>,
) -> bool {
    let destinations = &settings.mount_destinations;
    let mut keep = destinations.iter().any(|d| s_mount.destination.eq(d));

    if !keep {
        if let Some(mounts) = yaml_mounts {
            keep = mounts.iter().any(|m| m.mountPath.eq(&s_mount.destination));
        }
    }

    keep
}

fn adjust_termination_path(mount: &mut policy::KataMount, yaml_container: &pod::Container) {
    if mount.destination == "/dev/termination-log" {
        if let Some(path) = &yaml_container.terminationMessagePath {
            mount.destination.clone_from(path);
        }
    }
}

pub fn get_mount_and_storage(
    settings: &settings::Settings,
    p_mounts: &mut Vec<policy::KataMount>,
    storages: &mut Vec<agent::Storage>,
    yaml_volume: &volume::Volume,
    yaml_mount: &pod::VolumeMount,
) {
    debug!(
        "get_mount_and_storage: adding mount and storage for: {:?}",
        &yaml_volume
    );

    if let Some(emptyDir) = &yaml_volume.emptyDir {
        let settings_volumes = &settings.volumes;
        let mut volume: Option<&settings::EmptyDirVolume> = None;

        if let Some(medium) = &emptyDir.medium {
            if medium == "Memory" {
                volume = Some(&settings_volumes.emptyDir_memory);
            }
        }

        if volume.is_none() {
            volume = if settings.kata_config.confidential_guest {
                Some(&settings_volumes.confidential_emptyDir)
            } else {
                Some(&settings_volumes.emptyDir)
            }
        }

        get_empty_dir_mount_and_storage(settings, p_mounts, storages, yaml_mount, volume.unwrap());
    } else if yaml_volume.persistentVolumeClaim.is_some() || yaml_volume.azureFile.is_some() {
        get_shared_bind_mount(yaml_mount, p_mounts, "rprivate", "rw");
    } else if yaml_volume.hostPath.is_some() {
        get_host_path_mount(yaml_mount, yaml_volume, p_mounts);
    } else if yaml_volume.configMap.is_some() || yaml_volume.secret.is_some() {
        get_config_map_mount_and_storage(settings, p_mounts, storages, yaml_mount);
    } else if yaml_volume.projected.is_some() {
        get_shared_bind_mount(yaml_mount, p_mounts, "rprivate", "ro");
    } else if yaml_volume.downwardAPI.is_some() {
        get_downward_api_mount(yaml_mount, p_mounts);
    } else {
        todo!("Unsupported volume type {:?}", yaml_volume);
    }
}

fn get_empty_dir_mount_and_storage(
    settings: &settings::Settings,
    p_mounts: &mut Vec<policy::KataMount>,
    storages: &mut Vec<agent::Storage>,
    yaml_mount: &pod::VolumeMount,
    settings_empty_dir: &settings::EmptyDirVolume,
) {
    debug!("Settings emptyDir: {:?}", settings_empty_dir);

    if yaml_mount.subPathExpr.is_none() {
        storages.push(agent::Storage {
            driver: settings_empty_dir.driver.clone(),
            driver_options: Vec::new(),
            source: settings_empty_dir.source.clone(),
            fstype: settings_empty_dir.fstype.clone(),
            options: settings_empty_dir.options.clone(),
            mount_point: format!("{}{}$", &settings_empty_dir.mount_point, &yaml_mount.name),
            fs_group: protobuf::MessageField::none(),
            special_fields: ::protobuf::SpecialFields::new(),
        });
    }

    let source = if yaml_mount.subPathExpr.is_some() {
        let file_name = Path::new(&yaml_mount.mountPath).file_name().unwrap();
        let name = OsString::from(file_name).into_string().unwrap();
        format!("{}{name}$", &settings.volumes.configMap.mount_source)
    } else {
        format!("{}{}$", &settings_empty_dir.mount_source, &yaml_mount.name)
    };

    let mount_type = if yaml_mount.subPathExpr.is_some() {
        "bind"
    } else {
        &settings_empty_dir.mount_type
    };

    let access = match yaml_mount.readOnly {
        Some(true) => {
            debug!("setting read only access for emptyDir mount");
            "ro"
        }
        _ => "rw",
    };

    p_mounts.push(policy::KataMount {
        destination: yaml_mount.mountPath.to_string(),
        type_: mount_type.to_string(),
        source,
        options: vec![
            "rbind".to_string(),
            "rprivate".to_string(),
            access.to_string(),
        ],
    });
}

fn get_host_path_mount(
    yaml_mount: &pod::VolumeMount,
    yaml_volume: &volume::Volume,
    p_mounts: &mut Vec<policy::KataMount>,
) {
    let host_path = yaml_volume.hostPath.as_ref().unwrap().path.clone();
    let path = Path::new(&host_path);

    let mut biderectional = false;
    if let Some(mount_propagation) = &yaml_mount.mountPropagation {
        if mount_propagation.eq("Bidirectional") {
            debug!("get_host_path_mount: Bidirectional");
            biderectional = true;
        }
    }

    let access = match yaml_mount.readOnly {
        Some(true) => {
            debug!("setting read only access for host path mount");
            "ro"
        }
        _ => "rw",
    };
    // TODO:
    //
    // - When volume.hostPath.path: /dev/ttyS0
    //      "source": "/dev/ttyS0"
    // - When volume.hostPath.path: /tmp/results
    //      "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-results$"
    //
    // What is the reason for this source path difference in the Guest OS?
    if !path.starts_with("/dev/") && !path.starts_with("/sys/") {
        debug!("get_host_path_mount: calling get_shared_bind_mount");
        let propagation = if biderectional { "rshared" } else { "rprivate" };
        get_shared_bind_mount(yaml_mount, p_mounts, propagation, access);
    } else {
        let dest = yaml_mount.mountPath.clone();
        let type_ = "bind".to_string();
        let mount_option = if biderectional { "rshared" } else { "rprivate" };
        let options = vec![
            "rbind".to_string(),
            mount_option.to_string(),
            access.to_string(),
        ];

        if let Some(policy_mount) = p_mounts.iter_mut().find(|m| m.destination.eq(&dest)) {
            debug!("get_host_path_mount: updating dest = {dest}, source = {host_path}");
            policy_mount.type_ = type_;
            policy_mount.source = host_path;
            policy_mount.options = options;
        } else {
            debug!("get_host_path_mount: adding dest = {dest}, source = {host_path}");
            p_mounts.push(policy::KataMount {
                destination: dest,
                type_,
                source: host_path,
                options,
            });
        }
    }
}

fn get_config_map_mount_and_storage(
    settings: &settings::Settings,
    p_mounts: &mut Vec<policy::KataMount>,
    storages: &mut Vec<agent::Storage>,
    yaml_mount: &pod::VolumeMount,
) {
    let settings_volumes = &settings.volumes;
    let settings_config_map = if settings.kata_config.confidential_guest {
        &settings_volumes.confidential_configMap
    } else {
        &settings_volumes.configMap
    };
    debug!("Settings configMap: {:?}", settings_config_map);

    if !settings.kata_config.confidential_guest {
        let mount_path = Path::new(&yaml_mount.mountPath).file_name().unwrap();
        let mount_path_str = OsString::from(mount_path).into_string().unwrap();

        storages.push(agent::Storage {
            driver: settings_config_map.driver.clone(),
            driver_options: Vec::new(),
            source: format!("{}{}$", &settings_config_map.mount_source, &yaml_mount.name),
            fstype: settings_config_map.fstype.clone(),
            options: settings_config_map.options.clone(),
            mount_point: format!("{}{mount_path_str}$", &settings_config_map.mount_point),
            fs_group: protobuf::MessageField::none(),
            special_fields: ::protobuf::SpecialFields::new(),
        });
    }

    let file_name = Path::new(&yaml_mount.mountPath).file_name().unwrap();
    let name = OsString::from(file_name).into_string().unwrap();
    p_mounts.push(policy::KataMount {
        destination: yaml_mount.mountPath.clone(),
        type_: settings_config_map.mount_type.clone(),
        source: format!("{}{name}$", &settings_config_map.mount_point),
        options: settings_config_map.options.clone(),
    });
}

fn get_shared_bind_mount(
    yaml_mount: &pod::VolumeMount,
    p_mounts: &mut Vec<policy::KataMount>,
    propagation: &str,
    access: &str,
) {
    let mount_path = if let Some(byte_index) = str::rfind(&yaml_mount.mountPath, '/') {
        str::from_utf8(&yaml_mount.mountPath.as_bytes()[byte_index + 1..]).unwrap()
    } else {
        &yaml_mount.mountPath
    };
    let source = format!("$(sfprefix){mount_path}$");

    let dest = yaml_mount.mountPath.clone();
    let type_ = "bind".to_string();
    let options = vec![
        "rbind".to_string(),
        propagation.to_string(),
        access.to_string(),
    ];

    if let Some(policy_mount) = p_mounts.iter_mut().find(|m| m.destination.eq(&dest)) {
        debug!("get_shared_bind_mount: updating dest = {dest}, source = {source}");
        policy_mount.type_ = type_;
        policy_mount.source = source;
        policy_mount.options = options;
    } else {
        debug!("get_shared_bind_mount: adding dest = {dest}, source = {source}");
        p_mounts.push(policy::KataMount {
            destination: dest,
            type_,
            source,
            options,
        });
    }
}

fn get_downward_api_mount(yaml_mount: &pod::VolumeMount, p_mounts: &mut Vec<policy::KataMount>) {
    let mount_path = if let Some(byte_index) = str::rfind(&yaml_mount.mountPath, '/') {
        str::from_utf8(&yaml_mount.mountPath.as_bytes()[byte_index + 1..]).unwrap()
    } else {
        &yaml_mount.mountPath
    };
    let source = format!("$(sfprefix){mount_path}$");

    let dest = yaml_mount.mountPath.clone();
    let type_ = "bind".to_string();
    let options = vec![
        "rbind".to_string(),
        "rprivate".to_string(),
        "ro".to_string(),
    ];

    if let Some(policy_mount) = p_mounts.iter_mut().find(|m| m.destination.eq(&dest)) {
        debug!("get_downward_api_mount: updating dest = {dest}, source = {source}");
        policy_mount.type_ = type_;
        policy_mount.source = source;
        policy_mount.options = options;
    } else {
        debug!("get_downward_api_mount: adding dest = {dest}, source = {source}");
        p_mounts.push(policy::KataMount {
            destination: dest,
            type_,
            source,
            options,
        });
    }
}

pub fn get_image_mount_and_storage(
    settings: &settings::Settings,
    p_mounts: &mut Vec<policy::KataMount>,
    storages: &mut Vec<agent::Storage>,
    destination: &str,
) {
    // https://github.com/kubernetes/examples/blob/master/cassandra/image/Dockerfile
    // has a volume mount starting with two '/' characters:
    //
    // CASSANDRA_DATA=/cassandra_data
    // VOLUME ["/$CASSANDRA_DATA"]
    let mut destination_string = destination.to_string();
    while destination_string.contains("//") {
        destination_string = destination_string.replace("//", "/");
    }
    debug!("get_image_mount_and_storage: image dest = {destination}, dest = {destination_string}");

    for mount in &mut *p_mounts {
        if mount.destination == destination_string {
            debug!(
                "get_image_mount_and_storage: mount {destination_string} already defined by YAML"
            );
            return;
        }
    }

    let settings_image = &settings.volumes.image_volume;
    debug!(
        "get_image_mount_and_storage: settings for container image volumes: {:?}",
        settings_image
    );

    storages.push(agent::Storage {
        driver: settings_image.driver.clone(),
        driver_options: Vec::new(),
        source: settings_image.source.clone(),
        fstype: settings_image.fstype.clone(),
        options: settings_image.options.clone(),
        mount_point: destination_string.clone(),
        fs_group: protobuf::MessageField::none(),
        special_fields: ::protobuf::SpecialFields::new(),
    });

    let file_name = Path::new(&destination_string).file_name().unwrap();
    let name = OsString::from(file_name).into_string().unwrap();
    let source = format!("{}{name}$", &settings_image.mount_source);

    p_mounts.push(policy::KataMount {
        destination: destination_string,
        type_: settings_image.fstype.clone(),
        source,
        options: settings_image.options.clone(),
    });
}
