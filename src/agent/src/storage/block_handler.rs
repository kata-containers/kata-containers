// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::linux_abi::pcipath_from_dev_tree_path;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
#[cfg(target_arch = "s390x")]
use kata_types::device::DRIVER_BLK_CCW_TYPE;
use kata_types::device::{
    DRIVER_BLK_MMIO_TYPE, DRIVER_BLK_PCI_TYPE, DRIVER_NVDIMM_TYPE, DRIVER_SCSI_TYPE,
};
use kata_types::mount::{
    confidential_storage_mount_name, validate_confidential_manifest_uri, StorageDevice,
    KATA_BLOCK_VOLUME_CREATE_FS,
};
use nix::sys::stat::{major, minor};
use protocols::{
    agent::{ConfidentialStorageAccess, Storage},
    confidential_data_hub::VolumeAccess,
};
use tracing::instrument;

#[cfg(target_arch = "s390x")]
use crate::ccw;
#[cfg(target_arch = "s390x")]
use crate::device::block_device_handler::get_virtio_blk_ccw_device_name;
use crate::device::block_device_handler::{
    get_virtio_blk_mmio_device_name, get_virtio_blk_pci_device_name,
};
use crate::device::nvdimm_device_handler::wait_for_pmem_device;
use crate::device::scsi_device_handler::get_scsi_device_name;
use crate::storage::{
    common_storage_handler, new_device, set_ownership, StorageContext, StorageHandler,
};
use slog::Logger;
#[cfg(target_arch = "s390x")]
use std::str::FromStr;

const EPHEMERAL_ENCRYPTION_DRIVER_OPTION: &str = "encryption_key=ephemeral";
const CONFIDENTIAL_STORAGE_FSTYPE: &str = "confidential-storage";
const CONFIDENTIAL_MAPPER_PREFIX: &str = "/dev/mapper/coco-pv-";
const CONFIDENTIAL_MOUNT_PREFIX: &str = "/run/kata-containers/shared/containers/passthrough/";
const CONFIDENTIAL_EXT4_MOUNT_OPTIONS: [&str; 3] = ["nodev", "nosuid", "rw"];
const MKFS_EXT4: &str = "mkfs.ext4";
const BLOCK_EMPTYDIR_EXT4_MKFS_OPTS: [&str; 8] =
    ["-O", "^has_journal", "-m", "0", "-i", "163840", "-I", "128"];

#[derive(Debug, Eq, PartialEq)]
struct BlockStorageDriverOptions {
    has_ephemeral_encryption: bool,
    should_create_filesystem: bool,
    confidential_storage: Option<ConfidentialStorageDriverOptions>,
}

#[derive(Debug, Eq, PartialEq)]
struct ConfidentialStorageDriverOptions {
    manifest_uri: String,
    requested_access: VolumeAccess,
}

fn get_device_number(dev_path: &str, metadata: Option<&fs::Metadata>) -> Result<String> {
    let dev_id = match metadata {
        Some(m) => m.rdev(),
        None => {
            let m =
                fs::metadata(dev_path).context(format!("get metadata on file {:?}", dev_path))?;
            m.rdev()
        }
    };
    Ok(format!("{}:{}", major(dev_id), minor(dev_id)))
}

async fn handle_block_storage(
    logger: &Logger,
    storage: &Storage,
    dev_num: &str,
    sandbox: &Arc<tokio::sync::Mutex<crate::sandbox::Sandbox>>,
) -> Result<Arc<dyn StorageDevice>> {
    let options = block_storage_driver_options(storage)?;

    if let Some(confidential_storage) = options.confidential_storage {
        activate_confidential_storage(logger, storage, dev_num, &confidential_storage, sandbox)
            .await
    } else if options.has_ephemeral_encryption {
        let mkfs_opts = BLOCK_EMPTYDIR_EXT4_MKFS_OPTS.join(" ");
        crate::rpc::cdh_secure_mount(
            "block-device",
            dev_num,
            "luks2",
            &storage.mount_point,
            &mkfs_opts,
        )
        .await?;
        set_ownership(logger, storage)?;
        new_device(storage.mount_point.clone())
    } else {
        if options.should_create_filesystem {
            ensure_block_filesystem(logger, storage).await?;
        }
        let path = common_storage_handler(logger, storage)?;
        new_device(path)
    }
}

fn block_storage_driver_options(storage: &Storage) -> Result<BlockStorageDriverOptions> {
    let has_ephemeral_encryption = storage
        .driver_options
        .iter()
        .any(|opt| opt == EPHEMERAL_ENCRYPTION_DRIVER_OPTION);
    let should_create_filesystem = should_create_block_filesystem(storage);
    let confidential_storage = confidential_storage_driver_options(storage)?;

    if confidential_storage.is_some() && (has_ephemeral_encryption || should_create_filesystem) {
        return Err(anyhow!(
            "confidential storage cannot be combined with ephemeral encryption or host-requested filesystem creation"
        ));
    }

    if has_ephemeral_encryption && !should_create_filesystem {
        return Err(anyhow!(
            "{} requires {} for block storage",
            EPHEMERAL_ENCRYPTION_DRIVER_OPTION,
            KATA_BLOCK_VOLUME_CREATE_FS
        ));
    }

    Ok(BlockStorageDriverOptions {
        has_ephemeral_encryption,
        should_create_filesystem,
        confidential_storage,
    })
}

fn confidential_storage_driver_options(
    storage: &Storage,
) -> Result<Option<ConfidentialStorageDriverOptions>> {
    let Some(request) = storage.confidential_storage.as_ref() else {
        if storage.fstype == CONFIDENTIAL_STORAGE_FSTYPE {
            return Err(anyhow!(
                "confidential storage discriminator requires an activation request"
            ));
        }
        return Ok(None);
    };

    if storage.fstype != CONFIDENTIAL_STORAGE_FSTYPE
        || !storage.driver_options.is_empty()
        || !storage.options.is_empty()
        || storage.shared
    {
        return Err(anyhow!("invalid confidential storage mount contract"));
    }

    let requested_access = request
        .requested_access
        .enum_value()
        .map_err(|value| anyhow!("unknown confidential storage access value {value}"))?;
    if requested_access != ConfidentialStorageAccess::ReadWrite {
        return Err(anyhow!(
            "only readWrite confidential storage access is supported"
        ));
    }
    validate_confidential_manifest_uri(&request.manifest_uri)?;

    Ok(Some(ConfidentialStorageDriverOptions {
        manifest_uri: request.manifest_uri.clone(),
        requested_access: VolumeAccess::VOLUME_ACCESS_READ_WRITE,
    }))
}

/// Validate every invariant that can be checked before storage handling mutates the sandbox.
pub(crate) fn validate_confidential_storage_contract(storage: &Storage) -> Result<()> {
    let Some(options) = confidential_storage_driver_options(storage)? else {
        return Ok(());
    };

    if !matches!(
        storage.driver.as_str(),
        DRIVER_BLK_PCI_TYPE | DRIVER_SCSI_TYPE
    ) {
        return Err(anyhow!(
            "confidential storage requires a virtio-blk or virtio-scsi device"
        ));
    }

    let mount_name = confidential_storage_mount_name(&options.manifest_uri)?;
    let expected_mount_point = format!("{CONFIDENTIAL_MOUNT_PREFIX}{mount_name}");
    if storage.mount_point != expected_mount_point {
        return Err(anyhow!(
            "confidential storage mount point does not match its manifest"
        ));
    }

    if let Some(fs_group) = storage.fs_group.as_ref() {
        if fs_group
            .group_change_policy
            .enum_value()
            .map_or(true, |policy| {
                !matches!(
                    policy,
                    protocols::types::FSGroupChangePolicy::Always
                        | protocols::types::FSGroupChangePolicy::OnRootMismatch
                )
            })
        {
            return Err(anyhow!("invalid confidential storage fsGroup contract"));
        }
    }

    Ok(())
}

fn validate_activation(activation: &crate::confidential_data_hub::ActivatedVolume) -> Result<()> {
    validate_activation_fields(activation)?;
    let metadata = fs::metadata(&activation.device_path)
        .with_context(|| format!("inspect activated mapper {}", activation.device_path))?;
    if !metadata.file_type().is_block_device() {
        return Err(anyhow!("CDH activation path is not a block device"));
    }
    Ok(())
}

fn validate_activation_fields(
    activation: &crate::confidential_data_hub::ActivatedVolume,
) -> Result<()> {
    if activation.activation_id.is_empty()
        || activation.activation_id.len() > 128
        || !activation
            .activation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(anyhow!("CDH returned an invalid activation ID"));
    }
    if activation.effective_access != VolumeAccess::VOLUME_ACCESS_READ_WRITE {
        return Err(anyhow!(
            "CDH effective access does not match authorized readWrite access"
        ));
    }
    if !activation
        .device_path
        .starts_with(CONFIDENTIAL_MAPPER_PREFIX)
    {
        return Err(anyhow!(
            "CDH returned a mapper path outside the fixed profile"
        ));
    }
    let mapper_name = activation
        .device_path
        .strip_prefix("/dev/mapper/")
        .ok_or_else(|| anyhow!("CDH returned an invalid mapper path"))?;
    let mapper_suffix = activation
        .device_path
        .strip_prefix(CONFIDENTIAL_MAPPER_PREFIX)
        .ok_or_else(|| anyhow!("CDH returned an invalid mapper path"))?;
    if mapper_suffix.is_empty()
        || mapper_name.len() >= 128
        || !mapper_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(anyhow!("CDH returned an invalid mapper path"));
    }
    Ok(())
}

async fn deactivate_after_failure(activation_id: &str, mount_point: &str) -> Result<()> {
    if Path::new(mount_point).exists() {
        new_device(mount_point.to_string())?.cleanup()?;
    }
    crate::confidential_data_hub::deactivate_volume(activation_id).await
}

async fn activate_confidential_storage(
    logger: &Logger,
    storage: &Storage,
    dev_num: &str,
    options: &ConfidentialStorageDriverOptions,
    sandbox: &Arc<tokio::sync::Mutex<crate::sandbox::Sandbox>>,
) -> Result<Arc<dyn StorageDevice>> {
    let activation = crate::confidential_data_hub::activate_volume(
        dev_num,
        &options.manifest_uri,
        options.requested_access,
    )
    .await
    .with_context(|| format!("activate confidential block device {dev_num}"))?;

    if let Err(error) = validate_activation(&activation) {
        let _ = crate::confidential_data_hub::deactivate_volume(&activation.activation_id).await;
        return Err(error);
    }

    let mut mount = storage.clone();
    mount.source.clone_from(&activation.device_path);
    mount.fstype = "ext4".to_string();
    mount.options = CONFIDENTIAL_EXT4_MOUNT_OPTIONS
        .iter()
        .map(|option| option.to_string())
        .collect();
    mount.driver_options.clear();
    mount.confidential_storage = Default::default();

    let path = match common_storage_handler(logger, &mount) {
        Ok(path) => path,
        Err(error) => {
            let cleanup =
                deactivate_after_failure(&activation.activation_id, &storage.mount_point).await;
            return Err(error).context(format!(
                "mount activated confidential storage; cleanup result: {cleanup:?}"
            ));
        }
    };

    if let Err(error) = sandbox
        .lock()
        .await
        .register_confidential_storage_activation(&path, activation.activation_id.clone())
    {
        let cleanup = deactivate_after_failure(&activation.activation_id, &path).await;
        return Err(error).context(format!(
            "register confidential storage activation; cleanup result: {cleanup:?}"
        ));
    }

    new_device(path)
}

fn should_create_block_filesystem(storage: &Storage) -> bool {
    storage
        .driver_options
        .iter()
        .any(|opt| opt == KATA_BLOCK_VOLUME_CREATE_FS)
}

async fn ensure_block_filesystem(logger: &Logger, storage: &Storage) -> Result<()> {
    match storage.fstype.as_str() {
        "ext4" => ensure_ext4_filesystem(logger, &storage.source).await,
        _ => Err(anyhow!(
            "creating filesystem {} for block storage is unsupported",
            storage.fstype
        )),
    }
}

async fn ensure_ext4_filesystem(logger: &Logger, source: &str) -> Result<()> {
    // This option is emitted for block emptyDir volumes, whose backing device
    // is ephemeral and freshly allocated for the pod.
    info!(logger, "creating ext4 filesystem"; "source" => source);
    let output = {
        // Keep the agent SIGCHLD handler from reaping this child before
        // tokio::process observes it.
        let _locker = rustjail::container::WAIT_PID_LOCKER.lock().await;
        // BLOCK_EMPTYDIR_EXT4_MKFS_OPTS mirrors CDH's EXT4_INTEGRITY_MKFS_OPTS
        // from confidential-data-hub/hub/src/storage/volume_type/blockdevice/mod.rs.
        // CDH's FsFormatter adds "-F" and its mapped device path separately in
        // confidential-data-hub/hub/src/storage/drivers/filesystem.rs; here the
        // agent invokes mkfs.ext4 directly, so add "-F" and source below.
        tokio::process::Command::new(MKFS_EXT4)
            .arg("-F")
            .args(BLOCK_EMPTYDIR_EXT4_MKFS_OPTS)
            .arg(source)
            .output()
            .await
            .with_context(|| format!("run {MKFS_EXT4} for {source}"))?
    };

    if output.status.success() {
        return Ok(());
    }

    Err(anyhow!(
        "{} failed for {}: status={}, stdout={}, stderr={}",
        MKFS_EXT4,
        source,
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage_with_driver_options(options: &[&str]) -> Storage {
        Storage {
            driver_options: options.iter().map(|opt| opt.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn block_storage_options_allow_normal_existing_storage() {
        let storage = storage_with_driver_options(&[]);

        let options = block_storage_driver_options(&storage).unwrap();

        assert_eq!(
            options,
            BlockStorageDriverOptions {
                has_ephemeral_encryption: false,
                should_create_filesystem: false,
                confidential_storage: None,
            }
        );
    }

    #[test]
    fn block_storage_options_allow_plain_fresh_storage() {
        let storage = storage_with_driver_options(&[KATA_BLOCK_VOLUME_CREATE_FS]);

        let options = block_storage_driver_options(&storage).unwrap();

        assert_eq!(
            options,
            BlockStorageDriverOptions {
                has_ephemeral_encryption: false,
                should_create_filesystem: true,
                confidential_storage: None,
            }
        );
    }

    #[test]
    fn block_storage_options_allow_encrypted_fresh_storage() {
        let storage = storage_with_driver_options(&[
            EPHEMERAL_ENCRYPTION_DRIVER_OPTION,
            KATA_BLOCK_VOLUME_CREATE_FS,
        ]);

        let options = block_storage_driver_options(&storage).unwrap();

        assert_eq!(
            options,
            BlockStorageDriverOptions {
                has_ephemeral_encryption: true,
                should_create_filesystem: true,
                confidential_storage: None,
            }
        );
    }

    #[test]
    fn block_storage_options_reject_encryption_without_filesystem_creation() {
        let storage = storage_with_driver_options(&[EPHEMERAL_ENCRYPTION_DRIVER_OPTION]);

        let err = block_storage_driver_options(&storage).unwrap_err();

        assert!(err.to_string().contains(KATA_BLOCK_VOLUME_CREATE_FS));
    }

    fn confidential_storage(access: ConfidentialStorageAccess) -> Storage {
        let manifest_uri = "kbs:///tenant/storage-manifests/workspace-v1";
        Storage {
            driver: DRIVER_BLK_PCI_TYPE.to_string(),
            fstype: CONFIDENTIAL_STORAGE_FSTYPE.to_string(),
            mount_point: format!(
                "{CONFIDENTIAL_MOUNT_PREFIX}{}",
                confidential_storage_mount_name(manifest_uri).unwrap()
            ),
            confidential_storage: protobuf::MessageField::some(
                protocols::agent::ConfidentialStorage {
                    manifest_uri: manifest_uri.to_string(),
                    requested_access: protobuf::EnumOrUnknown::new(access),
                    ..Default::default()
                },
            ),
            ..Default::default()
        }
    }

    #[test]
    fn confidential_storage_options_accept_exact_read_write_contract() {
        let storage = confidential_storage(ConfidentialStorageAccess::ReadWrite);
        validate_confidential_storage_contract(&storage).unwrap();
        let options = block_storage_driver_options(&storage).unwrap();

        assert_eq!(
            options.confidential_storage,
            Some(ConfidentialStorageDriverOptions {
                manifest_uri: "kbs:///tenant/storage-manifests/workspace-v1".to_string(),
                requested_access: VolumeAccess::VOLUME_ACCESS_READ_WRITE,
            })
        );
    }

    #[test]
    fn confidential_storage_options_reject_downgrade_and_mixed_options() {
        let mut storage = confidential_storage(ConfidentialStorageAccess::ReadWrite);
        storage.fstype = "ext4".to_string();
        assert!(block_storage_driver_options(&storage).is_err());

        let mut storage = confidential_storage(ConfidentialStorageAccess::ReadWrite);
        storage
            .driver_options
            .push(KATA_BLOCK_VOLUME_CREATE_FS.to_string());
        assert!(block_storage_driver_options(&storage).is_err());

        assert!(block_storage_driver_options(&confidential_storage(
            ConfidentialStorageAccess::ReadOnly,
        ))
        .is_err());
    }

    #[test]
    fn confidential_storage_preflight_rejects_identity_and_fsgroup_substitution() {
        let mut wrong_mount = confidential_storage(ConfidentialStorageAccess::ReadWrite);
        wrong_mount.mount_point =
            format!("{CONFIDENTIAL_MOUNT_PREFIX}confidential-{}", "0".repeat(64));
        assert!(validate_confidential_storage_contract(&wrong_mount).is_err());

        let mut wrong_driver = confidential_storage(ConfidentialStorageAccess::ReadWrite);
        wrong_driver.driver = "local".to_string();
        assert!(validate_confidential_storage_contract(&wrong_driver).is_err());

        let mut root_group = confidential_storage(ConfidentialStorageAccess::ReadWrite);
        root_group.fs_group = protobuf::MessageField::some(protocols::agent::FSGroup {
            group_id: 0,
            ..Default::default()
        });
        assert!(validate_confidential_storage_contract(&root_group).is_ok());

        let mut unknown_group_policy = confidential_storage(ConfidentialStorageAccess::ReadWrite);
        unknown_group_policy.fs_group = protobuf::MessageField::some(protocols::agent::FSGroup {
            group_id: 3000,
            group_change_policy: protobuf::EnumOrUnknown::from_i32(99),
            ..Default::default()
        });
        assert!(validate_confidential_storage_contract(&unknown_group_policy).is_err());
    }

    #[test]
    fn validates_bounded_cdh_activation_fields() {
        let valid = crate::confidential_data_hub::ActivatedVolume {
            activation_id: "4f24103d-3754-4f65-a091-92fc9cab87cc".to_string(),
            device_path: "/dev/mapper/coco-pv-a_b-C9".to_string(),
            effective_access: VolumeAccess::VOLUME_ACCESS_READ_WRITE,
        };
        assert!(validate_activation_fields(&valid).is_ok());

        let mut outside_mapper = crate::confidential_data_hub::ActivatedVolume { ..valid };
        outside_mapper.device_path = "/dev/dm-0".to_string();
        assert!(validate_activation_fields(&outside_mapper).is_err());

        let invalid = [
            crate::confidential_data_hub::ActivatedVolume {
                activation_id: String::new(),
                ..outside_mapper
            },
            crate::confidential_data_hub::ActivatedVolume {
                activation_id: "activation/1".to_string(),
                device_path: "/dev/mapper/coco-pv-volume".to_string(),
                effective_access: VolumeAccess::VOLUME_ACCESS_READ_WRITE,
            },
            crate::confidential_data_hub::ActivatedVolume {
                activation_id: "activation-1".to_string(),
                device_path: CONFIDENTIAL_MAPPER_PREFIX.to_string(),
                effective_access: VolumeAccess::VOLUME_ACCESS_READ_WRITE,
            },
            crate::confidential_data_hub::ActivatedVolume {
                activation_id: "activation-1".to_string(),
                device_path: "/dev/mapper/coco-pv-volume".to_string(),
                effective_access: VolumeAccess::VOLUME_ACCESS_READ_ONLY,
            },
        ];
        for activation in invalid {
            assert!(validate_activation_fields(&activation).is_err());
        }
    }
}

#[derive(Debug)]
pub struct VirtioBlkMmioHandler {}

#[async_trait::async_trait]
impl StorageHandler for VirtioBlkMmioHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_MMIO_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        if !Path::new(&storage.source).exists() {
            get_virtio_blk_mmio_device_name(ctx.sandbox, &storage.source)
                .await
                .context("failed to get mmio device name")?;
        }
        let dev_num = get_device_number(&storage.source, None)?;
        handle_block_storage(ctx.logger, &storage, &dev_num, ctx.sandbox).await
    }
}

#[derive(Debug)]
pub struct VirtioBlkPciHandler {}

#[async_trait::async_trait]
impl StorageHandler for VirtioBlkPciHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_PCI_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let dev_num: String;

        // If hot-plugged, get the device node path based on the PCI path
        // otherwise use the virt path provided in Storage Source
        if storage.source.starts_with("/dev") {
            let metadata = fs::metadata(&storage.source)
                .context(format!("get metadata on file {:?}", &storage.source))?;
            let mode = metadata.permissions().mode();
            if mode & libc::S_IFBLK == 0 {
                return Err(anyhow!("Invalid device {}", &storage.source));
            }
            dev_num = get_device_number(&storage.source, Some(&metadata))?;
        } else {
            let (root_complex, pcipath) = pcipath_from_dev_tree_path(&storage.source)?;
            let dev_path =
                get_virtio_blk_pci_device_name(ctx.sandbox, root_complex, &pcipath).await?;
            storage.source = dev_path;
            dev_num = get_device_number(&storage.source, None)?;
        }

        handle_block_storage(ctx.logger, &storage, &dev_num, ctx.sandbox).await
    }
}

#[cfg(target_arch = "s390x")]
#[derive(Debug)]
pub struct VirtioBlkCcwHandler {}

#[cfg(target_arch = "s390x")]
#[async_trait::async_trait]
impl StorageHandler for VirtioBlkCcwHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_CCW_TYPE]
    }

    #[cfg(target_arch = "s390x")]
    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let ccw_device = ccw::Device::from_str(&storage.source)?;
        let dev_path = get_virtio_blk_ccw_device_name(ctx.sandbox, &ccw_device).await?;
        storage.source = dev_path;
        let dev_num = get_device_number(&storage.source, None)?;
        handle_block_storage(ctx.logger, &storage, &dev_num, ctx.sandbox).await
    }

    #[cfg(not(target_arch = "s390x"))]
    #[instrument]
    async fn create_device(
        &self,
        _storage: Storage,
        _ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        Err(anyhow!("CCW is only supported on s390x"))
    }
}

#[derive(Debug)]
pub struct ScsiHandler {}

#[async_trait::async_trait]
impl StorageHandler for ScsiHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_SCSI_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // Retrieve the device path from SCSI address.
        let dev_path = get_scsi_device_name(ctx.sandbox, &storage.source).await?;
        storage.source = dev_path.clone();

        let dev_num = get_device_number(&dev_path, None)?;
        handle_block_storage(ctx.logger, &storage, &dev_num, ctx.sandbox).await
    }
}

#[derive(Debug)]
pub struct PmemHandler {}

#[async_trait::async_trait]
impl StorageHandler for PmemHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_NVDIMM_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // Retrieve the device for pmem storage
        wait_for_pmem_device(ctx.sandbox, &storage.source).await?;

        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}
