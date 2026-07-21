// Copyright (c) 2022-2023 Alibaba Cloud
// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs,
    fs::OpenOptions,
    os::unix::{fs::OpenOptionsExt, io::AsRawFd},
    path::{Path, PathBuf},
};

use crate::{
    share_fs::{do_get_guest_path, do_get_host_path},
    volume::share_fs_volume::generate_mount_path,
};
use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount::{get_mount_options, get_mount_path};
use kata_types::device::{
    DRIVER_BLK_CCW_TYPE as KATA_CCW_DEV_TYPE, DRIVER_BLK_PCI_TYPE as KATA_BLK_DEV_TYPE,
    DRIVER_SCSI_TYPE as KATA_SCSI_DEV_TYPE,
};
use oci_spec::runtime as oci;

use hypervisor::device::DeviceType;

pub const DEFAULT_VOLUME_FS_TYPE: &str = "ext4";
pub const KATA_MOUNT_BIND_TYPE: &str = "bind";
pub const KATA_MOUNT_RBIND_TYPE: &str = "rbind";

/// Build the OCI mount options for the container-side bind mount of a direct
/// block volume.
pub(crate) fn build_bind_mount_options(
    volume_options: &[String],
    oci_options: &Option<Vec<String>>,
) -> Vec<String> {
    let mut opts = oci_options.clone().unwrap_or_default();
    opts.extend(volume_options.iter().cloned());

    if !opts
        .iter()
        .any(|o| matches!(o.as_str(), KATA_MOUNT_BIND_TYPE | KATA_MOUNT_RBIND_TYPE))
    {
        opts.push(KATA_MOUNT_BIND_TYPE.to_string());
    }
    opts
}

/// Filter OCI bind-mount flags ("bind", "rbind") from volume options that
/// will be passed to the agent's `Storage.options`.
pub(crate) fn filter_block_storage_options(options: &[String]) -> Vec<String> {
    options
        .iter()
        .filter(|o| !matches!(o.as_str(), KATA_MOUNT_BIND_TYPE | KATA_MOUNT_RBIND_TYPE))
        .cloned()
        .collect()
}

// BLKROGET (_IO(0x12, 94)) returns the block device's read-only flag into an
// int. It is encoded as an `_IO` ioctl but actually transfers data, so it is a
// "bad" ioctl; request_code_none! produces the correct, arch-aware value.
nix::ioctl_read_bad!(blkroget, nix::request_code_none!(0x12, 94), libc::c_int);

/// Query the host block device's read-only flag (BLKROGET). This reflects the
/// device's actual writability, which is the ground truth for whether the guest
/// should see it read-only: when the host backing is read-only, writes from the
/// guest fail at the host anyway, so the device must be exposed read-only. The
/// read-only intent for such devices is frequently not carried in the OCI spec
/// (no "ro" mount option), so the device flag is the only reliable source.
pub(crate) fn is_block_device_readonly<P: AsRef<Path>>(path: P) -> Result<bool> {
    let path = path.as_ref();
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("open {} for readonly probe", path.display()))?;

    let mut ro: libc::c_int = 0;
    // Safe: file owns a valid fd for the duration of the call and `ro` is a
    // valid, properly aligned pointer to an initialized int.
    unsafe { blkroget(file.as_raw_fd(), &mut ro).context("ioctl BLKROGET")? };

    Ok(ro != 0)
}

pub fn get_file_name<P: AsRef<Path>>(src: P) -> Result<String> {
    let file_name = src
        .as_ref()
        .file_name()
        .map(|v| v.to_os_string())
        .ok_or_else(|| {
            std::io::Error::other(format!(
                "failed to get file name of path {}",
                src.as_ref().to_string_lossy()
            ))
        })?
        .into_string()
        .map_err(|e| anyhow!("failed to convert to string {:?}", e))?;

    Ok(file_name)
}

pub(crate) async fn generate_shared_path(
    dest: PathBuf,
    read_only: bool,
    device_id: &str,
    sid: &str,
) -> Result<String> {
    let file_name = get_file_name(&dest).context("failed to get file name.")?;
    let mount_name = generate_mount_path(device_id, file_name.as_str());
    let guest_path = do_get_guest_path(&mount_name, device_id, true, false);
    // Note: directories should always be created under the rw/ path. The ro/ directory is a
    // read-only bind mount of rw/, so creating directories directly under ro/ would fail with a read-only FS.
    let host_path = do_get_host_path(&mount_name, sid, device_id, true, read_only);

    if get_mount_path(&Some(dest)).starts_with("/dev") {
        fs::File::create(&host_path).context(format!("failed to create file {:?}", &host_path))?;
    } else {
        std::fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create dir {}: {:?}", host_path, e))?;
    }

    Ok(guest_path)
}

/// Extract storage source information from block device configuration.
/// This helper function handles the common logic for determining the storage source
/// based on the driver type (BLK/SCSI/CCW).
fn extract_storage_source(
    driver_option: &str,
    pci_path: Option<&hypervisor::device::pci_path::PciPath>,
    scsi_addr: Option<&str>,
    ccw_addr: Option<&str>,
    virt_path: &str,
) -> Result<String> {
    let source = match driver_option {
        KATA_BLK_DEV_TYPE => {
            if let Some(pci_path) = pci_path {
                pci_path.to_string()
            } else {
                return Err(anyhow!("block driver is blk but no pci path exists"));
            }
        }
        KATA_SCSI_DEV_TYPE => {
            if let Some(scsi_addr) = scsi_addr {
                scsi_addr.to_string()
            } else {
                return Err(anyhow!("block driver is scsi but no scsi address exists"));
            }
        }
        KATA_CCW_DEV_TYPE => {
            if let Some(ccw_addr) = ccw_addr {
                ccw_addr.to_string()
            } else {
                return Err(anyhow!("block driver is ccw but no ccw address exists"));
            }
        }
        _ => virt_path.to_string(),
    };

    Ok(source)
}

pub async fn handle_block_volume(
    device_info: DeviceType,
    m: &oci::Mount,
    read_only: bool,
    sid: &str,
    fstype: &str,
    volume_options: Option<&[String]>,
) -> Result<(agent::Storage, oci::Mount, String)> {
    let oci_opts = get_mount_options(m.options());
    let mut storage_options: Vec<String> =
        filter_block_storage_options(volume_options.unwrap_or(&oci_opts));

    if read_only && !storage_options.iter().any(|o| o == "ro") {
        storage_options.push("ro".to_string());
    }

    let mut storage = agent::Storage {
        options: storage_options,
        ..Default::default()
    };

    // As the true Block Device wrapped in DeviceType, we need to
    // get it out from the wrapper, and the device_id will be for
    // BlockVolume.
    // safe here, device_info is correct and only unwrap it.
    let mut device_id = String::new();

    if let DeviceType::BlockModern(device_mod) = device_info.clone() {
        let device = &device_mod.lock().await;
        storage.driver = device.config.driver_option.clone();
        storage.source = extract_storage_source(
            &device.config.driver_option,
            device.config.pci_path.as_ref(),
            device.config.scsi_addr.as_deref(),
            device.config.ccw_addr.as_deref(),
            &device.config.virt_path,
        )?;
        device_id = device.device_id.clone();
    }

    // generate host guest shared path
    let guest_path = generate_shared_path(m.destination().clone(), read_only, &device_id, sid)
        .await
        .context("generate host-guest shared path failed")?;
    storage.mount_point = guest_path.clone();

    // In some case, dest is device /dev/xxx
    if m.destination()
        .clone()
        .display()
        .to_string()
        .starts_with("/dev")
    {
        storage.fs_type = "bind".to_string();
        storage.options.append(&mut get_mount_options(m.options()));
    } else {
        // usually, the dest is directory.
        storage.fs_type = fstype.to_owned();
    }

    // The Storage object already mounts the block device at `guest_path`
    // inside the guest. The OCI Mount then bind-mounts from `guest_path` to
    // the container's destination, so its type must be "bind" rather than a
    // filesystem type like "ext4".
    let mut mount = oci::Mount::default();
    mount.set_destination(m.destination().clone());
    mount.set_typ(Some(KATA_MOUNT_BIND_TYPE.to_string()));
    mount.set_source(Some(PathBuf::from(&guest_path)));
    mount.set_options(Some(build_bind_mount_options(volume_options.unwrap_or(&[]), m.options())));

    debug!(sl!(), "handle block volume with device_id: {}, storage: {:?}, mount: {:?}", device_id, storage, mount);

    Ok((storage, mount, device_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_bind_mount_options_merges_volume_options() {
        let volume_options = vec!["ro".to_string(), "nosuid".to_string()];
        let oci_options = Some(vec!["rbind".to_string(), "noexec".to_string()]);

        let opts = build_bind_mount_options(&volume_options, &oci_options);

        assert_eq!(
            opts,
            vec![
                "rbind".to_string(),
                "noexec".to_string(),
                "ro".to_string(),
                "nosuid".to_string(),
            ]
        );
    }

    #[test]
    fn test_build_bind_mount_options_with_oci_options() {
        let oci_options = Some(vec!["noexec".to_string(), "nodev".to_string()]);

        let opts = build_bind_mount_options(&[], &oci_options);

        // OCI options are used verbatim, then "bind" is appended because none of
        // them is a bind/rbind flag.
        assert_eq!(
            opts,
            vec![
                "noexec".to_string(),
                "nodev".to_string(),
                KATA_MOUNT_BIND_TYPE.to_string(),
            ]
        );
    }
}
