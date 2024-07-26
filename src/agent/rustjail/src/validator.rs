// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container::Config;
use anyhow::{anyhow, Context, Result};
use oci::{Linux, LinuxIdMapping, LinuxNamespace, Spec};
use oci_spec::runtime as oci;
use regex::Regex;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::{Component, PathBuf};

fn get_linux(oci: &Spec) -> Result<&Linux> {
    oci.linux()
        .as_ref()
        .ok_or_else(|| anyhow!("Unable to get Linux section from Spec"))
}

fn contain_namespace(nses: &[LinuxNamespace], key: &str) -> bool {
    let nstype = match oci::LinuxNamespaceType::try_from(key) {
        Ok(ns_type) => ns_type,
        Err(_e) => return false,
    };

    for ns in nses {
        if ns.typ() == nstype {
            return true;
        }
    }

    false
}

fn rootfs(root: &str) -> Result<()> {
    let path = PathBuf::from(root);
    // not absolute path or not exists
    if !path.exists() || !path.is_absolute() {
        return Err(anyhow!(
            "Path from {:?} does not exist or is not absolute",
            root
        ));
    }

    // symbolic link? ..?
    let mut stack: Vec<String> = Vec::new();
    for c in path.components() {
        if stack.is_empty() && (c == Component::RootDir || c == Component::ParentDir) {
            continue;
        }

        if c == Component::ParentDir {
            stack.pop();
            continue;
        }

        if let Some(v) = c.as_os_str().to_str() {
            stack.push(v.to_string());
        } else {
            return Err(anyhow!("Invalid path component (unable to convert to str)"));
        }
    }

    let mut cleaned = PathBuf::from("/");
    for e in stack.iter() {
        cleaned.push(e);
    }

    let canon = path.canonicalize().context("failed to canonicalize path")?;
    if cleaned != canon {
        // There is symbolic in path
        return Err(anyhow!(
            "There may be illegal symbols in the path name. Cleaned ({:?}) and canonicalized ({:?}) paths do not match",
            cleaned,
            canon));
    }

    Ok(())
}

fn hostname(oci: &Spec) -> Result<()> {
    if oci.hostname().is_none() {
        return Ok(());
    }

    let linux = get_linux(oci)?;
    let default_vec = vec![];
    if !contain_namespace(linux.namespaces().as_ref().unwrap_or(&default_vec), "uts") {
        return Err(anyhow!("Linux namespace does not contain uts"));
    }

    Ok(())
}

fn security(oci: &Spec) -> Result<()> {
    let linux = get_linux(oci)?;
    let label_pattern = r".*_u:.*_r:.*_t:s[0-9]|1[0-5].*";
    let label_regex = Regex::new(label_pattern)?;

    let default_vec = vec![];
    if let Some(process) = oci.process().as_ref() {
        if process.selinux_label().is_some()
            && !label_regex.is_match(process.selinux_label().as_ref().unwrap())
        {
            return Err(anyhow!(
                "SELinux label for the process is invalid format: {:?}",
                &process.selinux_label()
            ));
        }
    }
    if linux.mount_label().is_some() && !label_regex.is_match(linux.mount_label().as_ref().unwrap())
    {
        return Err(anyhow!(
            "SELinux label for the mount is invalid format: {}",
            linux.mount_label().as_ref().unwrap()
        ));
    }

    if linux.masked_paths().is_none() && linux.readonly_paths().is_none() {
        return Ok(());
    }

    if !contain_namespace(linux.namespaces().as_ref().unwrap_or(&default_vec), "mnt") {
        return Err(anyhow!("Linux namespace does not contain mount"));
    }

    Ok(())
}

fn idmapping(maps: &[LinuxIdMapping]) -> Result<()> {
    for map in maps {
        if map.size() > 0 {
            return Ok(());
        }
    }

    Err(anyhow!("No idmap has size > 0"))
}

fn usernamespace(oci: &Spec) -> Result<()> {
    let linux = get_linux(oci)?;

    let default_vec = vec![];
    if contain_namespace(linux.namespaces().as_ref().unwrap_or(&default_vec), "user") {
        let user_ns = PathBuf::from("/proc/self/ns/user");
        if !user_ns.exists() {
            return Err(anyhow!("user namespace not supported!"));
        }
        // check if idmappings is correct, at least I saw idmaps
        // with zero size was passed to agent
        let default_vec2 = vec![];
        idmapping(linux.uid_mappings().as_ref().unwrap_or(&default_vec2))
            .context("idmapping uid")?;
        idmapping(linux.gid_mappings().as_ref().unwrap_or(&default_vec2))
            .context("idmapping gid")?;
    } else {
        // no user namespace but idmap
        if !linux.uid_mappings().is_none() || !linux.gid_mappings().is_none() {
            return Err(anyhow!("No user namespace, but uid or gid mapping exists"));
        }
    }

    Ok(())
}

fn cgroupnamespace(oci: &Spec) -> Result<()> {
    let linux = get_linux(oci)?;

    let default_vec = vec![];
    if contain_namespace(
        linux.namespaces().as_ref().unwrap_or(&default_vec),
        "cgroup",
    ) {
        let path = PathBuf::from("/proc/self/ns/cgroup");
        if !path.exists() {
            return Err(anyhow!("cgroup unsupported!"));
        }
    }
    Ok(())
}

lazy_static! {
    pub static ref SYSCTLS: HashMap<&'static str, bool> = {
        let mut m = HashMap::new();
        m.insert("kernel.msgmax", true);
        m.insert("kernel.msgmnb", true);
        m.insert("kernel.msgmni", true);
        m.insert("kernel.sem", true);
        m.insert("kernel.shmall", true);
        m.insert("kernel.shmmax", true);
        m.insert("kernel.shmmni", true);
        m.insert("kernel.shm_rmid_forced", true);
        m
    };
}

fn sysctl(oci: &Spec) -> Result<()> {
    let linux = get_linux(oci)?;

    let default_hash = HashMap::new();
    let sysctl_hash = linux.sysctl().as_ref().unwrap_or(&default_hash);
    let default_vec = vec![];
    let linux_namespaces = linux.namespaces().as_ref().unwrap_or(&default_vec);
    for (key, _) in sysctl_hash.iter() {
        if SYSCTLS.contains_key(key.as_str()) || key.starts_with("fs.mqueue.") {
            if contain_namespace(linux_namespaces, "ipc") {
                continue;
            } else {
                return Err(anyhow!("Linux namespace does not contain ipc"));
            }
        }

        if key.starts_with("net.") {
            // the network ns is shared with the guest, don't expect to find it in spec
            continue;
        }

        if contain_namespace(linux_namespaces, "uts") {
            if key == "kernel.domainname" {
                continue;
            }

            if key == "kernel.hostname" {
                return Err(anyhow!("Kernel hostname specfied in Spec"));
            }
        }

        return Err(anyhow!("Sysctl config contains invalid settings"));
    }
    Ok(())
}

fn rootless_euid_mapping(oci: &Spec) -> Result<()> {
    let linux = get_linux(oci)?;

    let default_ns = vec![];
    if !contain_namespace(linux.namespaces().as_ref().unwrap_or(&default_ns), "user") {
        return Err(anyhow!("Linux namespace is missing user"));
    }

    if linux.uid_mappings().is_none() || linux.gid_mappings().is_none() {
        return Err(anyhow!(
            "Rootless containers require at least one UID/GID mapping"
        ));
    }

    Ok(())
}

fn has_idmapping(maps: &[LinuxIdMapping], id: u32) -> bool {
    for map in maps {
        if id >= map.container_id() && id < map.container_id() + map.size() {
            return true;
        }
    }
    false
}

fn rootless_euid_mount(oci: &Spec) -> Result<()> {
    let linux = get_linux(oci)?;

    let default_mounts = vec![];
    let oci_mounts = oci.mounts().as_ref().unwrap_or(&default_mounts);
    for mnt in oci_mounts.iter() {
        let default_options = vec![];
        let mnt_options = mnt.options().as_ref().unwrap_or(&default_options);
        for opt in mnt_options.iter() {
            if opt.starts_with("uid=") || opt.starts_with("gid=") {
                let fields: Vec<&str> = opt.split('=').collect();

                if fields.len() != 2 {
                    return Err(anyhow!("Options has invalid field: {:?}", fields));
                }

                let id = fields[1]
                    .trim()
                    .parse::<u32>()
                    .context(format!("parse field {}", &fields[1]))?;

                if opt.starts_with("uid=")
                    && !has_idmapping(linux.uid_mappings().as_ref().unwrap_or(&vec![]), id)
                {
                    return Err(anyhow!("uid of {} does not have a valid mapping", id));
                }

                if opt.starts_with("gid=")
                    && !has_idmapping(linux.gid_mappings().as_ref().unwrap_or(&vec![]), id)
                {
                    return Err(anyhow!("gid of {} does not have a valid mapping", id));
                }
            }
        }
    }
    Ok(())
}

fn rootless_euid(oci: &Spec) -> Result<()> {
    rootless_euid_mapping(oci).context("rootless euid mapping")?;
    rootless_euid_mount(oci).context("rotless euid mount")?;
    Ok(())
}

pub fn validate(conf: &Config) -> Result<()> {
    lazy_static::initialize(&SYSCTLS);
    let oci = conf
        .spec
        .as_ref()
        .ok_or_else(|| anyhow!("Invalid config spec"))?;

    if oci.linux().is_none() {
        return Err(anyhow!("oci Linux is none"));
    }

    let root = match oci.root().as_ref() {
        Some(v) => v.path().display().to_string(),
        None => return Err(anyhow!("oci root is none")),
    };

    rootfs(&root).context("rootfs")?;
    hostname(oci).context("hostname")?;
    security(oci).context("security")?;
    usernamespace(oci).context("usernamespace")?;
    cgroupnamespace(oci).context("cgroupnamespace")?;
    sysctl(oci).context("sysctl")?;

    if conf.rootless_euid {
        rootless_euid(oci).context("rootless euid")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci::{LinuxIdMappingBuilder, LinuxNamespaceBuilder, LinuxNamespaceType, Process, Spec};
    use oci_spec::runtime as oci;

    #[test]
    fn test_namespace() {
        let namespaces = [
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Network)
                .path("/sys/cgroups/net")
                .build()
                .unwrap(),
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Uts)
                .path("/sys/cgroups/uts")
                .build()
                .unwrap(),
        ];

        assert_eq!(contain_namespace(&namespaces, "net"), true);
        assert_eq!(contain_namespace(&namespaces, "uts"), true);

        assert_eq!(contain_namespace(&namespaces, ""), false);
        assert_eq!(contain_namespace(&namespaces, "Net"), false);
        assert_eq!(contain_namespace(&namespaces, "ipc"), false);
    }

    #[test]
    fn test_rootfs() {
        rootfs("/_no_exit_fs_xxxxxxxxxxx").unwrap_err();
        rootfs("sys").unwrap_err();
        rootfs("/proc/self/root").unwrap_err();
        rootfs("/proc/self/root/sys").unwrap_err();

        rootfs("/proc/self").unwrap_err();
        rootfs("/./proc/self").unwrap_err();
        rootfs("/proc/././self").unwrap_err();
        rootfs("/proc/.././self").unwrap_err();

        rootfs("/proc/uptime").unwrap();
        rootfs("/../proc/uptime").unwrap();
        rootfs("/../../proc/uptime").unwrap();
        rootfs("/proc/../proc/uptime").unwrap();
        rootfs("/proc/../../proc/uptime").unwrap();
    }

    #[test]
    fn test_hostname() {
        let mut spec = Spec::default();

        assert!(hostname(&spec).is_ok());

        spec.set_hostname(Some("a.test.com".to_owned()));
        assert!(hostname(&spec).is_ok());

        let mut linux = Linux::default();
        let namespaces = vec![
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Network)
                .path("/sys/cgroups/net")
                .build()
                .unwrap(),
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Uts)
                .path("/sys/cgroups/uts")
                .build()
                .unwrap(),
        ];
        linux.set_namespaces(Some(namespaces));
        spec.set_linux(Some(linux));
        assert!(hostname(&spec).is_ok());
    }

    #[test]
    fn test_security() {
        let mut spec = Spec::default();

        let linux = Linux::default();
        spec.set_linux(Some(linux));
        security(&spec).unwrap();

        let mut linux = Linux::default();
        linux.set_masked_paths(Some(vec!["/test".to_owned()]));
        let namespaces = vec![
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Network)
                .path("/sys/cgroups/net")
                .build()
                .unwrap(),
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Uts)
                .path("/sys/cgroups/uts")
                .build()
                .unwrap(),
        ];
        linux.set_namespaces(Some(namespaces));
        spec.set_linux(Some(linux));
        security(&spec).unwrap_err();

        let mut linux = Linux::default();
        linux.set_masked_paths(Some(vec!["/test".to_owned()]));
        let namespaces = vec![
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Network)
                .path("/sys/cgroups/net")
                .build()
                .unwrap(),
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Mount)
                .path("/sys/cgroups/mount")
                .build()
                .unwrap(),
        ];
        linux.set_namespaces(Some(namespaces));
        spec.set_linux(Some(linux));
        assert!(security(&spec).is_ok());

        // SELinux
        let valid_label = "system_u:system_r:container_t:s0:c123,c456";
        let mut process = Process::default();
        process.set_selinux_label(Some(valid_label.to_string()));
        spec.set_process(Some(process));
        security(&spec).unwrap();

        let mut linux = Linux::default();
        linux.set_mount_label(Some(valid_label.to_string()));
        spec.set_linux(Some(linux));
        security(&spec).unwrap();

        let invalid_label = "system_u:system_r:container_t";
        let mut process = Process::default();
        process.set_selinux_label(Some(invalid_label.to_string()));
        spec.set_process(Some(process));
        security(&spec).unwrap_err();

        let mut linux = Linux::default();
        linux.set_mount_label(Some(valid_label.to_string()));
        spec.set_linux(Some(linux));
        security(&spec).unwrap_err();
    }

    #[test]
    fn test_usernamespace() {
        let mut spec = Spec::default();
        assert!(usernamespace(&spec).is_ok());

        let linux = Linux::default();
        spec.set_linux(Some(linux));
        usernamespace(&spec).unwrap();

        let mut linux = Linux::default();

        let uidmap = LinuxIdMappingBuilder::default()
            .container_id(0u32)
            .host_id(1000u32)
            .size(0u32)
            .build()
            .unwrap();

        linux.set_uid_mappings(Some(vec![uidmap]));
        spec.set_linux(Some(linux));
        usernamespace(&spec).unwrap_err();
    }

    #[test]
    fn test_rootless_euid() {
        let mut spec = Spec::default();

        // Test case: without linux
        rootless_euid_mapping(&spec).unwrap_err();
        rootless_euid_mount(&spec).unwrap_err();

        // Test case: without user namespace
        let linux = Linux::default();
        spec.set_linux(Some(linux));
        rootless_euid_mapping(&spec).unwrap_err();

        // Test case: without user namespace
        let linux = spec.linux_mut().as_mut().unwrap();
        let namespaces = vec![
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Network)
                .path("/sys/cgroups/net")
                .build()
                .unwrap(),
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Uts)
                .path("/sys/cgroups/uts")
                .build()
                .unwrap(),
        ];
        linux.set_namespaces(Some(namespaces));
        rootless_euid_mapping(&spec).unwrap_err();

        let linux = spec.linux_mut().as_mut().unwrap();
        let namespaces = vec![
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Network)
                .path("/sys/cgroups/net")
                .build()
                .unwrap(),
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::User)
                .path("/sys/cgroups/user")
                .build()
                .unwrap(),
        ];
        linux.set_namespaces(Some(namespaces));

        let uidmap = LinuxIdMappingBuilder::default()
            .container_id(0u32)
            .host_id(1000u32)
            .size(1000u32)
            .build()
            .unwrap();
        let gidmap = LinuxIdMappingBuilder::default()
            .container_id(0u32)
            .host_id(1000u32)
            .size(1000u32)
            .build()
            .unwrap();

        linux.set_uid_mappings(Some(vec![uidmap]));
        linux.set_gid_mappings(Some(vec![gidmap]));
        rootless_euid_mapping(&spec).unwrap();

        let mut oci_mount = oci::Mount::default();
        oci_mount.set_destination("/app".into());
        oci_mount.set_typ(Some("tmpfs".to_owned()));
        oci_mount.set_source(Some("".into()));
        oci_mount.set_options(Some(vec!["uid=10000".to_owned()]));
        spec.mounts_mut().as_mut().unwrap().push(oci_mount);
        rootless_euid_mount(&spec).unwrap_err();

        let mut oci_mount = oci::Mount::default();
        oci_mount.set_destination("/app".into());
        oci_mount.set_typ(Some("tmpfs".to_owned()));
        oci_mount.set_source(Some("".into()));
        oci_mount.set_options(Some(vec!["uid=500".to_owned(), "gid=500".to_owned()]));
        spec.set_mounts(Some(vec![oci_mount]));

        rootless_euid(&spec).unwrap();
    }

    #[test]
    fn test_sysctl() {
        let mut spec = Spec::default();

        let mut linux = Linux::default();
        let namespaces = vec![LinuxNamespaceBuilder::default()
            .typ(LinuxNamespaceType::Network)
            .path("/sys/cgroups/net")
            .build()
            .unwrap()];
        linux.set_namespaces(Some(namespaces));

        let mut sysctl_hash = HashMap::new();
        sysctl_hash.insert("kernel.domainname".to_owned(), "test.com".to_owned());
        linux.set_sysctl(Some(sysctl_hash));

        spec.set_linux(Some(linux));
        sysctl(&spec).unwrap_err();

        spec.linux_mut()
            .as_mut()
            .unwrap()
            .namespaces_mut()
            .as_mut()
            .unwrap()
            .push(
                LinuxNamespaceBuilder::default()
                    .typ(LinuxNamespaceType::User)
                    .path("/sys/cgroups/user")
                    .build()
                    .unwrap(),
            );
        assert!(sysctl(&spec).is_err());
    }

    #[test]
    fn test_validate() {
        let spec = Spec::default();
        let mut config = Config {
            cgroup_name: "container1".to_owned(),
            use_systemd_cgroup: false,
            no_pivot_root: true,
            no_new_keyring: true,
            rootless_euid: false,
            rootless_cgroup: false,
            spec: Some(spec),
            container_name: "container1".to_owned(),
        };

        validate(&config).unwrap_err();

        let linux = Linux::default();
        config.spec.as_mut().unwrap().set_linux(Some(linux));
        validate(&config).unwrap_err();
    }
}
