// Copyright (c) 2022-2023 Alibaba Cloud
// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    share_fs::{do_get_guest_path, do_get_host_path},
    volume::share_fs_volume::generate_mount_path,
};
use anyhow::{anyhow, Context, Result};
use kata_sys_util::{
    eother,
    mount::{get_mount_options, get_mount_path},
};
use oci_spec::runtime as oci;

use hypervisor::device::DeviceType;

pub const DEFAULT_VOLUME_FS_TYPE: &str = "ext4";
pub const KATA_MOUNT_BIND_TYPE: &str = "bind";

pub const KATA_BLK_DEV_TYPE: &str = "blk";

pub fn get_file_name<P: AsRef<Path>>(src: P) -> Result<String> {
    let file_name = src
        .as_ref()
        .file_name()
        .map(|v| v.to_os_string())
        .ok_or_else(|| {
            eother!(
                "failed to get file name of path {}",
                src.as_ref().to_string_lossy()
            )
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
    let host_path = do_get_host_path(&mount_name, sid, device_id, true, read_only);

    if get_mount_path(&Some(dest)).starts_with("/dev") {
        fs::File::create(&host_path).context(format!("failed to create file {:?}", &host_path))?;
    } else {
        std::fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create dir {}: {:?}", host_path, e))?;
    }

    Ok(guest_path)
}

pub async fn handle_block_volume(
    device_info: DeviceType,
    m: &oci::Mount,
    read_only: bool,
    sid: &str,
    fstype: &str,
) -> Result<(agent::Storage, oci::Mount, String)> {
    // storage
    let mut storage = agent::Storage {
        options: if read_only {
            vec!["ro".to_string()]
        } else {
            Vec::new()
        },
        ..Default::default()
    };

    // As the true Block Device wrapped in DeviceType, we need to
    // get it out from the wrapper, and the device_id will be for
    // BlockVolume.
    // safe here, device_info is correct and only unwrap it.
    let mut device_id = String::new();
    if let DeviceType::Block(device) = device_info {
        let blk_driver = device.config.driver_option;
        // blk, mmioblk
        storage.driver = blk_driver.clone();
        storage.source = match blk_driver.as_str() {
            KATA_BLK_DEV_TYPE => {
                if let Some(pci_path) = device.config.pci_path {
                    pci_path.to_string()
                } else {
                    return Err(anyhow!("block driver is blk but no pci path exists"));
                }
            }
            _ => device.config.virt_path,
        };
        device_id = device.device_id;
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

    let mut mount = oci::Mount::default();
    mount.set_destination(m.destination().clone());
    mount.set_typ(Some(storage.fs_type.clone()));
    mount.set_source(Some(PathBuf::from(&guest_path)));
    mount.set_options(m.options().clone());

    Ok((storage, mount, device_id))
}
