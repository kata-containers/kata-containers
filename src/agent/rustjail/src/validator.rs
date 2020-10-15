// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container::Config;
use anyhow::{anyhow, Result};
use lazy_static;
use nix::errno::Errno;
use oci::{LinuxIDMapping, LinuxNamespace, Spec};
use std::collections::HashMap;
use std::path::{Component, PathBuf};

fn contain_namespace(nses: &Vec<LinuxNamespace>, key: &str) -> bool {
    for ns in nses {
        if ns.r#type.as_str() == key {
            return true;
        }
    }

    false
}

fn get_namespace_path(nses: &Vec<LinuxNamespace>, key: &str) -> Result<String> {
    for ns in nses {
        if ns.r#type.as_str() == key {
            return Ok(ns.path.clone());
        }
    }

    Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)))
}

fn rootfs(root: &str) -> Result<()> {
    let path = PathBuf::from(root);
    // not absolute path or not exists
    if !path.exists() || !path.is_absolute() {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    // symbolic link? ..?
    let mut stack: Vec<String> = Vec::new();
    for c in path.components() {
        if stack.is_empty() {
            if c == Component::RootDir || c == Component::ParentDir {
                continue;
            }
        }

        if c == Component::ParentDir {
            stack.pop();
            continue;
        }

        stack.push(c.as_os_str().to_str().unwrap().to_string());
    }

    let mut cleaned = PathBuf::from("/");
    for e in stack.iter() {
        cleaned.push(e);
    }

    let canon = path.canonicalize()?;
    if cleaned != canon {
        // There is symbolic in path
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    Ok(())
}

fn network(_oci: &Spec) -> Result<()> {
    Ok(())
}

fn hostname(oci: &Spec) -> Result<()> {
    if oci.hostname.is_empty() || oci.hostname == "".to_string() {
        return Ok(());
    }

    if oci.linux.is_none() {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }
    let linux = oci.linux.as_ref().unwrap();
    if !contain_namespace(&linux.namespaces, "uts") {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    Ok(())
}

fn security(oci: &Spec) -> Result<()> {
    let linux = oci.linux.as_ref().unwrap();
    if linux.masked_paths.len() == 0 && linux.readonly_paths.len() == 0 {
        return Ok(());
    }

    if !contain_namespace(&linux.namespaces, "mount") {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    // don't care about selinux at present

    Ok(())
}

fn idmapping(maps: &Vec<LinuxIDMapping>) -> Result<()> {
    for map in maps {
        if map.size > 0 {
            return Ok(());
        }
    }

    Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)))
}

fn usernamespace(oci: &Spec) -> Result<()> {
    let linux = oci.linux.as_ref().unwrap();
    if contain_namespace(&linux.namespaces, "user") {
        let user_ns = PathBuf::from("/proc/self/ns/user");
        if !user_ns.exists() {
            return Err(anyhow!("user namespace not supported!"));
        }
        // check if idmappings is correct, at least I saw idmaps
        // with zero size was passed to agent
        idmapping(&linux.uid_mappings)?;
        idmapping(&linux.gid_mappings)?;
    } else {
        // no user namespace but idmap
        if linux.uid_mappings.len() != 0 || linux.gid_mappings.len() != 0 {
            return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
        }
    }

    Ok(())
}

fn cgroupnamespace(oci: &Spec) -> Result<()> {
    let linux = oci.linux.as_ref().unwrap();
    if contain_namespace(&linux.namespaces, "cgroup") {
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

fn check_host_ns(path: &str) -> Result<()> {
    let cpath = PathBuf::from(path);
    let hpath = PathBuf::from("/proc/self/ns/net");

    let real_hpath = hpath.read_link()?;
    let meta = cpath.symlink_metadata()?;
    let file_type = meta.file_type();

    if !file_type.is_symlink() {
        return Ok(());
    }
    let real_cpath = cpath.read_link()?;
    if real_cpath == real_hpath {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    Ok(())
}

fn sysctl(oci: &Spec) -> Result<()> {
    let linux = oci.linux.as_ref().unwrap();
    for (key, _) in linux.sysctl.iter() {
        if SYSCTLS.contains_key(key.as_str()) || key.starts_with("fs.mqueue.") {
            if contain_namespace(&linux.namespaces, "ipc") {
                continue;
            } else {
                return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
            }
        }

        if key.starts_with("net.") {
            if !contain_namespace(&linux.namespaces, "network") {
                return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
            }

            let net = get_namespace_path(&linux.namespaces, "network")?;
            if net.is_empty() || net == "".to_string() {
                continue;
            }

            check_host_ns(net.as_str())?;
        }

        if contain_namespace(&linux.namespaces, "uts") {
            if key == "kernel.domainname" {
                continue;
            }

            if key == "kernel.hostname" {
                return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
            }
        }

        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }
    Ok(())
}

fn rootless_euid_mapping(oci: &Spec) -> Result<()> {
    let linux = oci.linux.as_ref().unwrap();
    if !contain_namespace(&linux.namespaces, "user") {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    if linux.uid_mappings.len() == 0 || linux.gid_mappings.len() == 0 {
        // rootless containers requires at least one UID/GID mapping
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    Ok(())
}

fn has_idmapping(maps: &Vec<LinuxIDMapping>, id: u32) -> bool {
    for map in maps {
        if id >= map.container_id && id < map.container_id + map.size {
            return true;
        }
    }
    false
}

fn rootless_euid_mount(oci: &Spec) -> Result<()> {
    let linux = oci.linux.as_ref().unwrap();

    for mnt in oci.mounts.iter() {
        for opt in mnt.options.iter() {
            if opt.starts_with("uid=") || opt.starts_with("gid=") {
                let fields: Vec<&str> = opt.split('=').collect();

                if fields.len() != 2 {
                    return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
                }

                let id = fields[1].trim().parse::<u32>()?;

                if opt.starts_with("uid=") {
                    if !has_idmapping(&linux.uid_mappings, id) {
                        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
                    }
                }

                if opt.starts_with("gid=") {
                    if !has_idmapping(&linux.gid_mappings, id) {
                        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
                    }
                }
            }
        }
    }
    Ok(())
}

fn rootless_euid(oci: &Spec) -> Result<()> {
    rootless_euid_mapping(oci)?;
    rootless_euid_mount(oci)?;
    Ok(())
}

pub fn validate(conf: &Config) -> Result<()> {
    lazy_static::initialize(&SYSCTLS);
    let oci = conf.spec.as_ref().unwrap();

    if oci.linux.is_none() {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    if oci.root.is_none() {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }
    let root = oci.root.as_ref().unwrap().path.as_str();

    rootfs(root)?;
    network(oci)?;
    hostname(oci)?;
    security(oci)?;
    usernamespace(oci)?;
    cgroupnamespace(oci)?;
    sysctl(&oci)?;

    if conf.rootless_euid {
        rootless_euid(oci)?;
    }

    Ok(())
}
