// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount::{create_mount_destination, parse_mount_options};
use kata_types::mount::{StorageDevice, StorageHandlerManager, KATA_SHAREDFS_GUEST_PREMOUNT_TAG};
use nix::unistd::{Gid, Uid};
use protocols::agent::Storage;
use protocols::types::FSGroupChangePolicy;
use slog::Logger;
use tokio::sync::Mutex;
use tracing::instrument;

use self::bind_watcher_handler::BindWatcherHandler;
use self::block_handler::{PmemHandler, ScsiHandler, VirtioBlkMmioHandler, VirtioBlkPciHandler};
use self::ephemeral_handler::EphemeralHandler;
use self::fs_handler::{OverlayfsHandler, Virtio9pHandler, VirtioFsHandler};
#[cfg(feature = "guest-pull")]
use self::image_pull_handler::ImagePullHandler;
use self::local_handler::LocalHandler;
use crate::mount::{baremount, is_mounted, remove_mounts};
use crate::sandbox::Sandbox;

pub use self::ephemeral_handler::update_ephemeral_mounts;

mod bind_watcher_handler;
mod block_handler;
mod ephemeral_handler;
mod fs_handler;
#[cfg(feature = "guest-pull")]
mod image_pull_handler;
mod local_handler;

const RW_MASK: u32 = 0o660;
const RO_MASK: u32 = 0o440;
const EXEC_MASK: u32 = 0o110;
const MODE_SETGID: u32 = 0o2000;

#[derive(Debug)]
pub struct StorageContext<'a> {
    cid: &'a Option<String>,
    logger: &'a Logger,
    sandbox: &'a Arc<Mutex<Sandbox>>,
}

/// An implementation of generic storage device.
#[derive(Default, Debug)]
pub struct StorageDeviceGeneric {
    path: Option<String>,
}

impl StorageDeviceGeneric {
    /// Create a new instance of `StorageStateCommon`.
    pub fn new(path: String) -> Self {
        StorageDeviceGeneric { path: Some(path) }
    }
}

impl StorageDevice for StorageDeviceGeneric {
    fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    fn cleanup(&self) -> Result<()> {
        let path = match self.path() {
            None => return Ok(()),
            Some(v) => {
                if v.is_empty() {
                    // TODO: Bind watch, local, ephemeral volume has empty path, which will get leaked.
                    return Ok(());
                } else {
                    v
                }
            }
        };
        if !Path::new(path).exists() {
            return Ok(());
        }

        if matches!(is_mounted(path), Ok(true)) {
            let mounts = vec![path.to_string()];
            remove_mounts(&mounts)?;
        }
        if matches!(is_mounted(path), Ok(true)) {
            return Err(anyhow!("failed to umount mountpoint {}", path));
        }

        let p = Path::new(path);
        if p.is_dir() {
            let is_empty = p.read_dir()?.next().is_none();
            if !is_empty {
                return Err(anyhow!("directory is not empty when clean up storage"));
            }
            // "remove_dir" will fail if the mount point is backed by a read-only filesystem.
            // This is the case with the device mapper snapshotter, where we mount the block device
            // directly at the underlying sandbox path which was provided from the base RO kataShared
            // path from the host.
            let _ = fs::remove_dir(p);
        } else if !p.is_file() {
            // TODO: should we remove the file for bind mount?
            return Err(anyhow!(
                "storage path {} is neither directory nor file",
                path
            ));
        }

        Ok(())
    }
}

/// Trait object to handle storage device.
#[async_trait::async_trait]
pub trait StorageHandler: Send + Sync {
    /// Create a new storage device.
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>>;

    /// Return the driver types that the handler manages.
    fn driver_types(&self) -> &[&str];
}

#[rustfmt::skip]
lazy_static! {
    pub static ref STORAGE_HANDLERS: StorageHandlerManager<Arc<dyn StorageHandler>> = {
        let mut manager: StorageHandlerManager<Arc<dyn StorageHandler>> = StorageHandlerManager::new();
        let handlers: Vec<Arc<dyn StorageHandler>> = vec![
            Arc::new(Virtio9pHandler {}),
            Arc::new(VirtioBlkMmioHandler {}),
            Arc::new(VirtioBlkPciHandler {}),
            Arc::new(EphemeralHandler {}),
            Arc::new(LocalHandler {}),
            Arc::new(PmemHandler {}),
            Arc::new(OverlayfsHandler {}),
            Arc::new(ScsiHandler {}),
            Arc::new(VirtioFsHandler {}),
            Arc::new(BindWatcherHandler {}),
            #[cfg(target_arch = "s390x")]
            Arc::new(self::block_handler::VirtioBlkCcwHandler {}),
            #[cfg(feature = "guest-pull")]
            Arc::new(ImagePullHandler {}),
        ];

        for handler in handlers {
            manager.add_handler(handler.driver_types(), handler.clone()).unwrap();
        }

        manager
    };
}

// add_storages takes a list of storages passed by the caller, and perform the
// associated operations such as waiting for the device to show up, and mount
// it to a specific location, according to the type of handler chosen, and for
// each storage.
#[instrument]
pub async fn add_storages(
    logger: Logger,
    storages: Vec<Storage>,
    sandbox: &Arc<Mutex<Sandbox>>,
    cid: Option<String>,
) -> Result<Vec<String>> {
    let mut mount_list = Vec::new();

    for storage in storages {
        let path = storage.mount_point.clone();
        let state = sandbox.lock().await.add_sandbox_storage(&path).await;
        if state.ref_count().await > 1 {
            if let Some(path) = state.path() {
                if !path.is_empty() {
                    mount_list.push(path.to_string());
                }
            }
            // The device already exists.
            continue;
        }

        if let Some(handler) = STORAGE_HANDLERS.handler(&storage.driver) {
            let logger =
                logger.new(o!( "subsystem" => "storage", "storage-type" => storage.driver.clone()));
            let mut ctx = StorageContext {
                cid: &cid,
                logger: &logger,
                sandbox,
            };

            match handler.create_device(storage, &mut ctx).await {
                Ok(device) => {
                    match sandbox
                        .lock()
                        .await
                        .update_sandbox_storage(&path, device.clone())
                    {
                        Ok(d) => {
                            if let Some(path) = device.path() {
                                if !path.is_empty() {
                                    mount_list.push(path.to_string());
                                }
                            }
                            drop(d);
                        }
                        Err(device) => {
                            error!(logger, "failed to update device for storage");
                            if let Err(e) = sandbox.lock().await.remove_sandbox_storage(&path).await
                            {
                                warn!(logger, "failed to remove dummy sandbox storage {:?}", e);
                            }
                            if let Err(e) = device.cleanup() {
                                error!(
                                    logger,
                                    "failed to clean state for storage device {}, {}", path, e
                                );
                            }
                            return Err(anyhow!("failed to update device for storage"));
                        }
                    }
                }
                Err(e) => {
                    error!(logger, "failed to create device for storage, error: {e:?}");
                    if let Err(e) = sandbox.lock().await.remove_sandbox_storage(&path).await {
                        warn!(logger, "failed to remove dummy sandbox storage {e:?}");
                    }
                    return Err(e);
                }
            }
        } else {
            return Err(anyhow!(
                "Failed to find the storage handler {}",
                storage.driver
            ));
        }
    }

    Ok(mount_list)
}

pub(crate) fn new_device(path: String) -> Result<Arc<dyn StorageDevice>> {
    let device = StorageDeviceGeneric::new(path);
    Ok(Arc::new(device))
}

#[instrument]
pub(crate) fn common_storage_handler(logger: &Logger, storage: &Storage) -> Result<String> {
    mount_storage(logger, storage)?;
    set_ownership(logger, storage)?;
    Ok(storage.mount_point.clone())
}

// mount_storage performs the mount described by the storage structure.
#[instrument]
fn mount_storage(logger: &Logger, storage: &Storage) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "mount"));

    // There's a special mechanism to create mountpoint from a `sharedfs` instance before
    // starting the kata-agent. Check for such cases.
    if storage.source == KATA_SHAREDFS_GUEST_PREMOUNT_TAG && is_mounted(&storage.mount_point)? {
        warn!(
            logger,
            "{} already mounted on {}, ignoring...",
            KATA_SHAREDFS_GUEST_PREMOUNT_TAG,
            &storage.mount_point
        );
        return Ok(());
    }

    let (flags, options) = parse_mount_options(&storage.options)?;
    let mount_path = Path::new(&storage.mount_point);
    let src_path = Path::new(&storage.source);
    create_mount_destination(src_path, mount_path, "", &storage.fstype)
        .context("Could not create mountpoint")?;

    info!(logger, "mounting storage";
        "mount-source" => src_path.display(),
        "mount-destination" => mount_path.display(),
        "mount-fstype"  => storage.fstype.as_str(),
        "mount-options" => options.as_str(),
    );

    baremount(
        src_path,
        mount_path,
        storage.fstype.as_str(),
        flags,
        options.as_str(),
        &logger,
    )
}

#[instrument]
pub(crate) fn parse_options(option_list: &[String]) -> HashMap<String, String> {
    let mut options = HashMap::new();
    for opt in option_list {
        let fields: Vec<&str> = opt.split('=').collect();
        if fields.len() == 2 {
            options.insert(fields[0].to_string(), fields[1].to_string());
        }
    }
    options
}

#[instrument]
pub fn set_ownership(logger: &Logger, storage: &Storage) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "mount", "fn" => "set_ownership"));

    // If fsGroup is not set, skip performing ownership change
    if storage.fs_group.is_none() {
        return Ok(());
    }

    let fs_group = storage.fs_group();
    let read_only = storage.options.contains(&String::from("ro"));
    let mount_path = Path::new(&storage.mount_point);
    let metadata = mount_path.metadata().map_err(|err| {
        error!(logger, "failed to obtain metadata for mount path";
            "mount-path" => mount_path.to_str(),
            "error" => err.to_string(),
        );
        err
    })?;

    if fs_group.group_change_policy == FSGroupChangePolicy::OnRootMismatch.into()
        && metadata.gid() == fs_group.group_id
    {
        let mut mask = if read_only { RO_MASK } else { RW_MASK };
        mask |= EXEC_MASK;

        // With fsGroup change policy to OnRootMismatch, if the current
        // gid of the mount path root directory matches the desired gid
        // and the current permission of mount path root directory is correct,
        // then ownership change will be skipped.
        let current_mode = metadata.permissions().mode();
        if (mask & current_mode == mask) && (current_mode & MODE_SETGID != 0) {
            info!(logger, "skipping ownership change for volume";
                "mount-path" => mount_path.to_str(),
                "fs-group" => fs_group.group_id.to_string(),
            );
            return Ok(());
        }
    }

    info!(logger, "performing recursive ownership change";
        "mount-path" => mount_path.to_str(),
        "fs-group" => fs_group.group_id.to_string(),
    );
    recursive_ownership_change(
        mount_path,
        None,
        Some(Gid::from_raw(fs_group.group_id)),
        read_only,
    )
}

#[instrument]
pub fn recursive_ownership_change(
    path: &Path,
    uid: Option<Uid>,
    gid: Option<Gid>,
    read_only: bool,
) -> Result<()> {
    let mut mask = if read_only { RO_MASK } else { RW_MASK };
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            recursive_ownership_change(entry?.path().as_path(), uid, gid, read_only)?;
        }
        mask |= EXEC_MASK;
        mask |= MODE_SETGID;
    }

    // We do not want to change the permission of the underlying file
    // using symlink. Hence we skip symlinks from recursive ownership
    // and permission changes.
    if path.is_symlink() {
        return Ok(());
    }

    nix::unistd::chown(path, uid, gid)?;

    if gid.is_some() {
        let metadata = path.metadata()?;
        let mut permission = metadata.permissions();
        let target_mode = metadata.mode() | mask;
        permission.set_mode(target_mode);
        fs::set_permissions(path, permission)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Error;
    use nix::mount::MsFlags;
    use protocols::agent::FSGroup;
    use std::fs::File;
    use tempfile::{tempdir, Builder};
    use test_utils::{
        skip_if_not_root, skip_loop_by_user, skip_loop_if_not_root, skip_loop_if_root, TestUserType,
    };

    #[test]
    fn test_mount_storage() {
        #[derive(Debug)]
        struct TestData<'a> {
            test_user: TestUserType,
            storage: Storage,
            error_contains: &'a str,

            make_source_dir: bool,
            make_mount_dir: bool,
            deny_mount_permission: bool,
        }

        impl Default for TestData<'_> {
            fn default() -> Self {
                TestData {
                    test_user: TestUserType::Any,
                    storage: Storage {
                        mount_point: "mnt".to_string(),
                        source: "src".to_string(),
                        fstype: "tmpfs".to_string(),
                        ..Default::default()
                    },
                    make_source_dir: true,
                    make_mount_dir: false,
                    deny_mount_permission: false,
                    error_contains: "",
                }
            }
        }

        let tests = &[
            TestData {
                test_user: TestUserType::NonRootOnly,
                error_contains: "EPERM: Operation not permitted",
                ..Default::default()
            },
            TestData {
                test_user: TestUserType::RootOnly,
                ..Default::default()
            },
            TestData {
                storage: Storage {
                    mount_point: "mnt".to_string(),
                    source: "src".to_string(),
                    fstype: "bind".to_string(),
                    ..Default::default()
                },
                make_source_dir: false,
                make_mount_dir: true,
                error_contains: "Could not create mountpoint",
                ..Default::default()
            },
            TestData {
                test_user: TestUserType::NonRootOnly,
                deny_mount_permission: true,
                error_contains: "Could not create mountpoint",
                ..Default::default()
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            skip_loop_by_user!(msg, d.test_user);

            let drain = slog::Discard;
            let logger = slog::Logger::root(drain, o!());

            let tempdir = tempdir().unwrap();

            let source = tempdir.path().join(&d.storage.source);
            let mount_point = tempdir.path().join(&d.storage.mount_point);

            let storage = Storage {
                source: source.to_str().unwrap().to_string(),
                mount_point: mount_point.to_str().unwrap().to_string(),
                ..d.storage.clone()
            };

            if d.make_source_dir {
                fs::create_dir_all(&storage.source).unwrap();
            }
            if d.make_mount_dir {
                fs::create_dir_all(&storage.mount_point).unwrap();
            }

            if d.deny_mount_permission {
                fs::set_permissions(
                    mount_point.parent().unwrap(),
                    fs::Permissions::from_mode(0o000),
                )
                .unwrap();
            }

            let result = mount_storage(&logger, &storage);

            // restore permissions so tempdir can be cleaned up
            if d.deny_mount_permission {
                fs::set_permissions(
                    mount_point.parent().unwrap(),
                    fs::Permissions::from_mode(0o755),
                )
                .unwrap();
            }

            if result.is_ok() {
                nix::mount::umount(&mount_point).unwrap();
            }

            let msg = format!("{}: result: {:?}", msg, result);
            if d.error_contains.is_empty() {
                assert!(result.is_ok(), "{}", msg);
            } else {
                assert!(result.is_err(), "{}", msg);
                let error_msg = format!("{}", result.unwrap_err());
                assert!(error_msg.contains(d.error_contains), "{}", msg);
            }
        }
    }

    #[test]
    fn test_set_ownership() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());

        #[derive(Debug)]
        struct TestData<'a> {
            mount_path: &'a str,
            fs_group: Option<FSGroup>,
            read_only: bool,
            expected_group_id: u32,
            expected_permission: u32,
        }

        let tests = &[
            TestData {
                mount_path: "foo",
                fs_group: None,
                read_only: false,
                expected_group_id: 0,
                expected_permission: 0,
            },
            TestData {
                mount_path: "rw_mount",
                fs_group: Some(FSGroup {
                    group_id: 3000,
                    group_change_policy: FSGroupChangePolicy::Always.into(),
                    ..Default::default()
                }),
                read_only: false,
                expected_group_id: 3000,
                expected_permission: RW_MASK | EXEC_MASK | MODE_SETGID,
            },
            TestData {
                mount_path: "ro_mount",
                fs_group: Some(FSGroup {
                    group_id: 3000,
                    group_change_policy: FSGroupChangePolicy::OnRootMismatch.into(),
                    ..Default::default()
                }),
                read_only: true,
                expected_group_id: 3000,
                expected_permission: RO_MASK | EXEC_MASK | MODE_SETGID,
            },
        ];

        let tempdir = tempdir().expect("failed to create tmpdir");

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let mount_dir = tempdir.path().join(d.mount_path);
            fs::create_dir(&mount_dir)
                .unwrap_or_else(|_| panic!("{}: failed to create root directory", msg));

            let directory_mode = mount_dir.as_path().metadata().unwrap().permissions().mode();
            let mut storage_data = Storage::new();
            if d.read_only {
                storage_data.set_options(vec!["foo".to_string(), "ro".to_string()]);
            }
            if let Some(fs_group) = d.fs_group.clone() {
                storage_data.set_fs_group(fs_group);
            }
            storage_data.mount_point = mount_dir.clone().into_os_string().into_string().unwrap();

            let result = set_ownership(&logger, &storage_data);
            assert!(result.is_ok());

            assert_eq!(
                mount_dir.as_path().metadata().unwrap().gid(),
                d.expected_group_id
            );
            assert_eq!(
                mount_dir.as_path().metadata().unwrap().permissions().mode(),
                (directory_mode | d.expected_permission)
            );
        }
    }

    #[test]
    fn test_recursive_ownership_change() {
        skip_if_not_root!();

        const COUNT: usize = 5;

        #[derive(Debug)]
        struct TestData<'a> {
            // Directory where the recursive ownership change should be performed on
            path: &'a str,

            // User ID for ownership change
            uid: u32,

            // Group ID for ownership change
            gid: u32,

            // Set when the permission should be read-only
            read_only: bool,

            // The expected permission of all directories after ownership change
            expected_permission_directory: u32,

            // The expected permission of all files after ownership change
            expected_permission_file: u32,
        }

        let tests = &[
            TestData {
                path: "no_gid_change",
                uid: 0,
                gid: 0,
                read_only: false,
                expected_permission_directory: 0,
                expected_permission_file: 0,
            },
            TestData {
                path: "rw_gid_change",
                uid: 0,
                gid: 3000,
                read_only: false,
                expected_permission_directory: RW_MASK | EXEC_MASK | MODE_SETGID,
                expected_permission_file: RW_MASK,
            },
            TestData {
                path: "ro_gid_change",
                uid: 0,
                gid: 3000,
                read_only: true,
                expected_permission_directory: RO_MASK | EXEC_MASK | MODE_SETGID,
                expected_permission_file: RO_MASK,
            },
        ];

        let tempdir = tempdir().expect("failed to create tmpdir");

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let mount_dir = tempdir.path().join(d.path);
            fs::create_dir(&mount_dir)
                .unwrap_or_else(|_| panic!("{}: failed to create root directory", msg));

            let directory_mode = mount_dir.as_path().metadata().unwrap().permissions().mode();
            let mut file_mode: u32 = 0;

            // create testing directories and files
            for n in 1..COUNT {
                let nest_dir = mount_dir.join(format!("nested{}", n));
                fs::create_dir(&nest_dir)
                    .unwrap_or_else(|_| panic!("{}: failed to create nest directory", msg));

                for f in 1..COUNT {
                    let filename = nest_dir.join(format!("file{}", f));
                    File::create(&filename)
                        .unwrap_or_else(|_| panic!("{}: failed to create file", msg));
                    file_mode = filename.as_path().metadata().unwrap().permissions().mode();
                }
            }

            let uid = if d.uid > 0 {
                Some(Uid::from_raw(d.uid))
            } else {
                None
            };
            let gid = if d.gid > 0 {
                Some(Gid::from_raw(d.gid))
            } else {
                None
            };
            let result = recursive_ownership_change(&mount_dir, uid, gid, d.read_only);

            assert!(result.is_ok());

            assert_eq!(mount_dir.as_path().metadata().unwrap().gid(), d.gid);
            assert_eq!(
                mount_dir.as_path().metadata().unwrap().permissions().mode(),
                (directory_mode | d.expected_permission_directory)
            );

            for n in 1..COUNT {
                let nest_dir = mount_dir.join(format!("nested{}", n));
                for f in 1..COUNT {
                    let filename = nest_dir.join(format!("file{}", f));
                    let file = Path::new(&filename);

                    assert_eq!(file.metadata().unwrap().gid(), d.gid);
                    assert_eq!(
                        file.metadata().unwrap().permissions().mode(),
                        (file_mode | d.expected_permission_file)
                    );
                }

                let dir = Path::new(&nest_dir);
                assert_eq!(dir.metadata().unwrap().gid(), d.gid);
                assert_eq!(
                    dir.metadata().unwrap().permissions().mode(),
                    (directory_mode | d.expected_permission_directory)
                );
            }
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn cleanup_storage() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());

        let tmpdir = Builder::new().tempdir().unwrap();
        let tmpdir_path = tmpdir.path().to_str().unwrap();

        let srcdir = Builder::new()
            .prefix("src")
            .tempdir_in(tmpdir_path)
            .unwrap();
        let srcdir_path = srcdir.path().to_str().unwrap();
        let empty_file = Path::new(srcdir_path).join("emptyfile");
        fs::write(&empty_file, "test").unwrap();

        let destdir = Builder::new()
            .prefix("dest")
            .tempdir_in(tmpdir_path)
            .unwrap();
        let destdir_path = destdir.path().to_str().unwrap();

        let emptydir = Builder::new()
            .prefix("empty")
            .tempdir_in(tmpdir_path)
            .unwrap();

        let s = StorageDeviceGeneric::default();
        assert!(s.cleanup().is_ok());

        let s = StorageDeviceGeneric::new("".to_string());
        assert!(s.cleanup().is_ok());

        let invalid_dir = emptydir
            .path()
            .join("invalid")
            .to_str()
            .unwrap()
            .to_string();
        let s = StorageDeviceGeneric::new(invalid_dir);
        assert!(s.cleanup().is_ok());

        assert!(bind_mount(srcdir_path, destdir_path, &logger).is_ok());

        let s = StorageDeviceGeneric::new(destdir_path.to_string());
        assert!(s.cleanup().is_ok());

        // fail to remove non-empty directory
        let s = StorageDeviceGeneric::new(srcdir_path.to_string());
        s.cleanup().unwrap_err();

        // remove a directory without umount
        fs::remove_file(&empty_file).unwrap();
        s.cleanup().unwrap();
    }

    fn bind_mount(src: &str, dst: &str, logger: &Logger) -> Result<(), Error> {
        let src_path = Path::new(src);
        let dst_path = Path::new(dst);

        baremount(src_path, dst_path, "bind", MsFlags::MS_BIND, "", logger)
    }
}
