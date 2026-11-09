// Copyright (c) 2026 Tencent. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount;
use kata_types::vmdk::{validate_vmdk_layout, VmdkConfig};
use nix::mount::{mount as mount_fs, umount2, MntFlags, MsFlags};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub(super) const VMDK_DESCRIPTOR: &str = "disk.vmdk";
pub(super) const VMDK_PLACEHOLDER: &str = "vmdk-placeholder";

// Bind opened FDs beside the descriptor for Firecracker and to avoid path races.
pub(super) fn prepare_vmdk(vmdk: &VmdkConfig, directory: &Path) -> Result<()> {
    let layout = validate_vmdk_layout(vmdk)?;
    let mut files: HashMap<&str, (File, String)> = HashMap::new();
    for extent in &vmdk.extents {
        if files.contains_key(extent.path_on_host.as_str()) {
            continue;
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&extent.path_on_host)
            .with_context(|| format!("open VMDK extent {}", extent.path_on_host))?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(anyhow!("VMDK extent must be a regular file"));
        }
        let required = layout
            .required_sectors_for(&extent.path_on_host)
            .ok_or_else(|| anyhow!("missing VMDK extent size"))?;
        if metadata.len() / 512 < required {
            return Err(anyhow!("VMDK extent is shorter than its declared layout"));
        }
        files.insert(
            &extent.path_on_host,
            (file, format!("extent-{}", files.len())),
        );
    }
    let descriptor = layout.render_descriptor_with(|extent| {
        files
            .get(extent.path_on_host.as_str())
            .map(|(_, name)| name.clone())
            .ok_or_else(|| anyhow!("missing prepared VMDK extent"))
    })?;

    fs::create_dir(directory).context("create Firecracker VMDK directory")?;
    let result = (|| {
        for (file, name) in files.values() {
            let source = format!("/proc/self/fd/{}", file.as_raw_fd());
            let target = directory.join(name);
            File::create(&target)?;
            mount_fs(
                Some(source.as_str()),
                &target,
                None::<&str>,
                MsFlags::MS_BIND,
                None::<&str>,
            )
            .context("bind mount VMDK extent")?;
            mount_fs(
                None::<&str>,
                &target,
                None::<&str>,
                MsFlags::MS_SLAVE,
                None::<&str>,
            )?;
            mount::bind_remount(&target, true).context("remount VMDK extent read-only")?;
        }
        fs::write(directory.join(VMDK_DESCRIPTOR), descriptor)
            .context("write Firecracker VMDK descriptor")
    })();
    if result.is_err() {
        cleanup_vmdk(directory).context("roll back Firecracker VMDK sources")?;
    }
    result
}

pub(super) fn cleanup_vmdk(directory: &Path) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("extent-") {
            match umount2(&entry.path(), MntFlags::MNT_DETACH) {
                Ok(()) | Err(nix::errno::Errno::EINVAL) => (),
                Err(error) => return Err(error).context("unmount Firecracker VMDK extent"),
            }
        }
    }
    fs::remove_dir_all(directory).context("remove Firecracker VMDK directory")
}

pub(super) fn create_vmdk_placeholder(directory: &Path) -> Result<()> {
    fs::create_dir_all(directory)?;
    fs::write(directory.join("empty.raw"), [0_u8; 512])?;
    let mut vmdk = VmdkConfig::default();
    vmdk.push_extent("empty.raw", 1, 0);
    let descriptor = validate_vmdk_layout(&vmdk)?
        .render_descriptor_with(|extent| Ok(extent.path_on_host.clone()))?;
    fs::write(directory.join(VMDK_DESCRIPTOR), descriptor)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_and_non_regular_extents_before_staging() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::write(&source, [0_u8; 512]).unwrap();
        let directory = temp.path().join("vmdk");
        let mut config = VmdkConfig::default();
        config.push_extent(source.to_str().unwrap(), 1, 1);
        assert!(prepare_vmdk(&config, &directory)
            .unwrap_err()
            .to_string()
            .contains("shorter"));
        config.extents[0].path_on_host = temp.path().display().to_string();
        assert!(prepare_vmdk(&config, &directory)
            .unwrap_err()
            .to_string()
            .contains("regular file"));
        assert!(!directory.exists());
    }

    #[test]
    fn stages_unique_readonly_extents_and_cleans_up() {
        if !nix::unistd::geteuid().is_root() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("host image");
        fs::write(&source, [0_u8; 1024]).unwrap();
        let mut config = VmdkConfig::default();
        config.push_extent(source.to_str().unwrap(), 1, 0);
        config.push_extent(source.to_str().unwrap(), 1, 1);
        let directory = temp.path().join("vmdk");
        prepare_vmdk(&config, &directory).unwrap();
        let descriptor = fs::read_to_string(directory.join(VMDK_DESCRIPTOR)).unwrap();
        assert!(descriptor.contains("RW 1 FLAT \"extent-0\" 0"));
        assert!(descriptor.contains("RW 1 FLAT \"extent-0\" 1"));
        assert!(OpenOptions::new()
            .write(true)
            .open(directory.join("extent-0"))
            .is_err());
        cleanup_vmdk(&directory).unwrap();
        assert!(!directory.exists());
        assert_eq!(fs::metadata(source).unwrap().len(), 1024);
    }
}
