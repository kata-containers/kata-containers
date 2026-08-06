// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::linux_abi::pcipath_from_dev_tree_path;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
#[cfg(target_arch = "s390x")]
use kata_types::device::DRIVER_BLK_CCW_TYPE;
use kata_types::device::{
    DRIVER_BLK_MMIO_TYPE, DRIVER_BLK_PCI_TYPE, DRIVER_NVDIMM_TYPE, DRIVER_SCSI_TYPE,
};
use kata_types::mount::{StorageDevice, KATA_BLOCK_VOLUME_CREATE_FS};
use nix::sys::stat::{major, minor};
use protocols::agent::{ConfidentialStorageProfile, Storage};
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
    volume_id: String,
    key_uri: String,
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
) -> Result<Arc<dyn StorageDevice>> {
    let options = block_storage_driver_options(storage)?;

    if let Some(confidential_storage) = options.confidential_storage {
        crate::rpc::cdh_confidential_storage_mount(
            "block-device",
            dev_num,
            &storage.mount_point,
            &confidential_storage.volume_id,
            &confidential_storage.key_uri,
        )
        .await?;
        set_ownership(logger, storage)?;
        new_device(storage.mount_point.clone())
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
    let measured_claim = storage
        .confidential_storage
        .as_ref()
        .and_then(|request| crate::initdata::confidential_storage_claim(&request.volume_id));
    block_storage_driver_options_with_claim(storage, measured_claim)
}

fn block_storage_driver_options_with_claim(
    storage: &Storage,
    measured_claim: Option<&crate::initdata::ConfidentialStorageClaim>,
) -> Result<BlockStorageDriverOptions> {
    let has_ephemeral_encryption = storage
        .driver_options
        .iter()
        .any(|opt| opt == EPHEMERAL_ENCRYPTION_DRIVER_OPTION);
    let should_create_filesystem = should_create_block_filesystem(storage);
    let confidential_storage = confidential_storage_driver_options(storage, measured_claim)?;

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
    measured_claim: Option<&crate::initdata::ConfidentialStorageClaim>,
) -> Result<Option<ConfidentialStorageDriverOptions>> {
    let Some(request) = storage.confidential_storage.as_ref() else {
        return Ok(None);
    };
    if request
        .profile
        .enum_value()
        .map_err(|value| anyhow!("unknown confidential storage profile enum value {value}"))?
        != ConfidentialStorageProfile::Luks2IntegrityExt4
    {
        return Err(anyhow!("unsupported confidential storage profile"));
    }
    if storage.fstype != CONFIDENTIAL_STORAGE_FSTYPE
        || !storage.options.is_empty()
        || storage.shared
    {
        return Err(anyhow!("invalid confidential storage mount contract"));
    }
    if !storage.driver_options.is_empty() {
        return Err(anyhow!(
            "confidential storage does not accept legacy driver options"
        ));
    }

    let measured_claim = measured_claim
        .ok_or_else(|| anyhow!("confidential storage request is absent from measured init-data"))?;
    if measured_claim.profile != crate::initdata::CONFIDENTIAL_STORAGE_PROFILE_LUKS2_INTEGRITY_EXT4
        || request.volume_id != measured_claim.volume_id
        || request.key_uri != measured_claim.key_uri
    {
        return Err(anyhow!(
            "confidential storage request does not match measured init-data"
        ));
    }

    Ok(Some(ConfidentialStorageDriverOptions {
        volume_id: request.volume_id.clone(),
        key_uri: request.key_uri.clone(),
    }))
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

    const TEST_VOLUME_ID: &str = "tenant/workload/volume";
    const TEST_KEY_URI: &str = "kbs:///tenant/storage/key";

    fn confidential_storage() -> Storage {
        Storage {
            fstype: CONFIDENTIAL_STORAGE_FSTYPE.to_string(),
            confidential_storage: protobuf::MessageField::some(
                protocols::agent::ConfidentialStorage {
                    profile: protobuf::EnumOrUnknown::new(
                        ConfidentialStorageProfile::Luks2IntegrityExt4,
                    ),
                    volume_id: TEST_VOLUME_ID.to_string(),
                    key_uri: TEST_KEY_URI.to_string(),
                    ..Default::default()
                },
            ),
            ..Default::default()
        }
    }

    fn measured_claim() -> crate::initdata::ConfidentialStorageClaim {
        crate::initdata::ConfidentialStorageClaim {
            profile: crate::initdata::CONFIDENTIAL_STORAGE_PROFILE_LUKS2_INTEGRITY_EXT4.to_string(),
            volume_id: TEST_VOLUME_ID.to_string(),
            key_uri: TEST_KEY_URI.to_string(),
        }
    }

    #[test]
    fn block_storage_options_allow_confidential_storage_bound_to_initdata() {
        let storage = confidential_storage();
        let claim = measured_claim();

        let options = block_storage_driver_options_with_claim(&storage, Some(&claim)).unwrap();

        assert_eq!(
            options,
            BlockStorageDriverOptions {
                has_ephemeral_encryption: false,
                should_create_filesystem: false,
                confidential_storage: Some(ConfidentialStorageDriverOptions {
                    volume_id: TEST_VOLUME_ID.to_string(),
                    key_uri: TEST_KEY_URI.to_string(),
                }),
            }
        );
    }

    #[test]
    fn block_storage_options_reject_confidential_storage_without_matching_initdata() {
        let storage = confidential_storage();

        assert!(block_storage_driver_options_with_claim(&storage, None).is_err());
        let mut mismatched = measured_claim();
        mismatched.key_uri = "kbs:///tenant/storage/other".to_string();
        assert!(block_storage_driver_options_with_claim(&storage, Some(&mismatched)).is_err());
    }

    #[test]
    fn block_storage_options_reject_invalid_confidential_contracts() {
        let mut cases = Vec::new();

        let mut unspecified = confidential_storage();
        unspecified
            .confidential_storage
            .mut_or_insert_default()
            .profile = protobuf::EnumOrUnknown::new(ConfidentialStorageProfile::Unspecified);
        cases.push(unspecified);

        let mut unknown_profile = confidential_storage();
        unknown_profile
            .confidential_storage
            .mut_or_insert_default()
            .profile = protobuf::EnumOrUnknown::from_i32(99);
        cases.push(unknown_profile);

        let mut plain_ext4_downgrade = confidential_storage();
        plain_ext4_downgrade.fstype = "ext4".to_string();
        cases.push(plain_ext4_downgrade);

        let mut mount_options = confidential_storage();
        mount_options.options.push("ro".to_string());
        cases.push(mount_options);

        let mut shared = confidential_storage();
        shared.shared = true;
        cases.push(shared);

        let mut mixed_ephemeral = confidential_storage();
        mixed_ephemeral
            .driver_options
            .push(EPHEMERAL_ENCRYPTION_DRIVER_OPTION.to_string());
        cases.push(mixed_ephemeral);

        let mut mixed_create = confidential_storage();
        mixed_create
            .driver_options
            .push(KATA_BLOCK_VOLUME_CREATE_FS.to_string());
        cases.push(mixed_create);

        let claim = measured_claim();
        for storage in cases {
            assert!(block_storage_driver_options_with_claim(&storage, Some(&claim)).is_err());
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
        handle_block_storage(ctx.logger, &storage, &dev_num).await
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

        handle_block_storage(ctx.logger, &storage, &dev_num).await
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
        handle_block_storage(ctx.logger, &storage, &dev_num).await
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
        handle_block_storage(ctx.logger, &storage, &dev_num).await
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
