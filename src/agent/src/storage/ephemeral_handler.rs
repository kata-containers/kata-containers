// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::BTreeSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount::parse_mount_options;
use kata_types::mount::{StorageDevice, KATA_MOUNT_OPTION_FS_GID};
use nix::unistd::Gid;
use protocols::agent::Storage;
use slog::Logger;
use tokio::sync::Mutex;
use tracing::instrument;

use crate::mount::baremount;
use crate::sandbox::Sandbox;
use crate::storage::{
    common_storage_handler, new_device, parse_options, StorageContext, StorageHandler, MODE_SETGID,
};
use kata_types::device::DRIVER_EPHEMERAL_TYPE;

const FS_TYPE_HUGETLB: &str = "hugetlbfs";
const FS_GID_EQ: &str = "fsgid=";
const SYS_FS_HUGEPAGES_PREFIX: &str = "/sys/kernel/mm/hugepages";

// A container's size= is its whole reservation for that page size, however many
// volumes it mounts, so each (container, page size) adds its pages once.
static HUGEPAGE_RESERVATIONS: std::sync::Mutex<BTreeSet<(String, u64)>> =
    std::sync::Mutex::new(BTreeSet::new());

fn read_nr_hugepages(path: &Path) -> Result<u64> {
    let text = fs::read_to_string(path).context(format!("reading {:?}", path))?;
    text.trim_end()
        .parse::<u64>()
        .map_err(|_| anyhow!("Unexpected text {:?} in {:?}", text, path))
}

fn write_nr_hugepages(path: &Path, pages: u64) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .context(format!("open nr_hugepages directory {:?}", path))?;
    file.write_all(pages.to_string().as_bytes())
        .context(format!("write nr_hugepages failed: {:?}", path))
}

/// Pages one storage added to the pool, so a failed mount can take them back.
#[derive(Debug)]
struct HugepageReservation {
    nr_hugepages: PathBuf,
    pages: u64,
    key: Option<(String, u64)>,
}

impl HugepageReservation {
    fn release(self) -> Result<()> {
        let mut reserved = HUGEPAGE_RESERVATIONS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let current = read_nr_hugepages(&self.nr_hugepages)?;
        write_nr_hugepages(&self.nr_hugepages, current.saturating_sub(self.pages))?;
        if let Some(key) = &self.key {
            reserved.remove(key);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct EphemeralHandler {}

#[async_trait::async_trait]
impl StorageHandler for EphemeralHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_EPHEMERAL_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // hugetlbfs
        if storage.fstype == FS_TYPE_HUGETLB {
            info!(ctx.logger, "handle hugetlbfs storage");
            // Allocate hugepages before mount
            // /sys/kernel/mm/hugepages/hugepages-1048576kB/nr_hugepages
            // /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages
            // options eg "pagesize=2097152,size=524288000"(2M, 500M)
            let reservation = Self::allocate_hugepages(
                ctx.logger,
                &storage.options.to_vec(),
                ctx.cid,
                Path::new(SYS_FS_HUGEPAGES_PREFIX),
            )
            .context("allocate hugepages")?;
            if let Err(e) = common_storage_handler(ctx.logger, &storage) {
                if let Some(Err(err)) = reservation.map(HugepageReservation::release) {
                    warn!(ctx.logger, "failed to release hugepages"; "error" => ?err);
                }
                return Err(e);
            }
        } else if !storage.options.is_empty() {
            // By now we only support one option field: "fsGroup" which
            // isn't an valid mount option, thus we should remove it when
            // do mount.
            let opts = parse_options(&storage.options);
            storage.options = Default::default();
            common_storage_handler(ctx.logger, &storage)?;

            // ephemeral_storage didn't support mount options except fsGroup.
            if let Some(fsgid) = opts.get(KATA_MOUNT_OPTION_FS_GID) {
                let gid = fsgid.parse::<u32>()?;

                nix::unistd::chown(storage.mount_point.as_str(), None, Some(Gid::from_raw(gid)))?;

                let meta = fs::metadata(&storage.mount_point)?;
                let mut permission = meta.permissions();

                let o_mode = meta.mode() | MODE_SETGID;
                permission.set_mode(o_mode);
                fs::set_permissions(&storage.mount_point, permission)?;
            }
        } else {
            common_storage_handler(ctx.logger, &storage)?;
        }

        new_device("".to_string())
    }
}

impl EphemeralHandler {
    // Add a container's reservation to the pool by writing to sysfs, so the
    // pool is the sum over the pod's containers instead of the last one.
    fn allocate_hugepages(
        logger: &Logger,
        options: &[String],
        cid: &Option<String>,
        sysfs: &Path,
    ) -> Result<Option<HugepageReservation>> {
        info!(logger, "mounting hugePages storage options: {:?}", options);

        let (pagesize, size) = Self::get_pagesize_and_size_from_option(options)
            .context(format!("parse mount options: {:?}", options))?;

        info!(
            logger,
            "allocate hugepages. pageSize: {}, size: {}", pagesize, size
        );

        // sysfs entry is always of the form hugepages-${pagesize}kB
        // Ref: https://www.kernel.org/doc/Documentation/vm/hugetlbpage.txt
        let path = sysfs
            .join(format!("hugepages-{}kB", pagesize / 1024))
            .join("nr_hugepages");

        let mut reserved = HUGEPAGE_RESERVATIONS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let key = cid.clone().map(|cid| (cid, pagesize));
        if key.as_ref().is_some_and(|k| reserved.contains(k)) {
            info!(logger, "the container's hugepages are already in the pool");
            return Ok(None);
        }

        let pages = size / pagesize;
        let current = read_nr_hugepages(&path)?;
        let target = current
            .checked_add(pages)
            .ok_or_else(|| anyhow!("too many hugepages of size {}", pagesize))?;
        info!(logger, "write {} pages to {:?}", target, &path);
        write_nr_hugepages(&path, target)?;

        // Even if the write succeeds, the kernel isn't guaranteed to be
        // able to allocate all the pages we requested.  Verify that it
        // did.
        let shortfall = match read_nr_hugepages(&path) {
            Ok(allocated) if allocated == target => None,
            Ok(allocated) => Some(anyhow!(
                "Only allocated {} of {} hugepages of size {}",
                allocated,
                target,
                pagesize
            )),
            Err(e) => Some(e),
        };
        if let Some(e) = shortfall {
            if let Err(err) = write_nr_hugepages(&path, current) {
                warn!(logger, "failed to restore nr_hugepages"; "error" => ?err);
            }
            return Err(e);
        }

        if let Some(key) = &key {
            reserved.insert(key.clone());
        }
        Ok(Some(HugepageReservation {
            nr_hugepages: path,
            pages,
            key,
        }))
    }

    // Parse filesystem options string to retrieve hugepage details
    // options eg "pagesize=2048,size=107374182"
    fn get_pagesize_and_size_from_option(options: &[String]) -> Result<(u64, u64)> {
        let mut pagesize_str: Option<&str> = None;
        let mut size_str: Option<&str> = None;

        for option in options {
            let vars: Vec<&str> = option.trim().split(',').collect();

            for var in vars {
                if let Some(stripped) = var.strip_prefix("pagesize=") {
                    pagesize_str = Some(stripped);
                } else if let Some(stripped) = var.strip_prefix("size=") {
                    size_str = Some(stripped);
                }

                if pagesize_str.is_some() && size_str.is_some() {
                    break;
                }
            }
        }

        if pagesize_str.is_none() || size_str.is_none() {
            return Err(anyhow!("no pagesize/size options found"));
        }

        let pagesize = pagesize_str
            .unwrap()
            .parse::<u64>()
            .context(format!("parse pagesize: {:?}", pagesize_str))?;
        let size = size_str
            .unwrap()
            .parse::<u64>()
            .context(format!("parse size: {:?}", size_str))?;

        Ok((pagesize, size))
    }
}

// update_ephemeral_mounts takes a list of ephemeral mounts and remounts them
// with mount options passed by the caller
#[instrument]
pub async fn update_ephemeral_mounts(
    logger: Logger,
    storages: &[Storage],
    _sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    for storage in storages {
        let handler_name = &storage.driver;
        let logger = logger.new(o!(
            "msg" => "updating tmpfs storage",
            "subsystem" => "storage",
            "storage-type" => handler_name.to_owned()));

        match handler_name.as_str() {
            DRIVER_EPHEMERAL_TYPE => {
                fs::create_dir_all(&storage.mount_point)?;

                if storage.options.is_empty() {
                    continue;
                } else {
                    // assume that fsGid has already been set
                    let mount_path = Path::new(&storage.mount_point);
                    let src_path = Path::new(&storage.source);
                    let opts: Vec<&String> = storage
                        .options
                        .iter()
                        .filter(|&opt| !opt.starts_with(FS_GID_EQ))
                        .collect();
                    let (flags, options) = parse_mount_options(&opts)?;

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
                    )?;
                }
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported storage type for syncing mounts {}. Only ephemeral storage update is supported",
                    storage.driver
                ));
            }
        };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_pagesize_and_size_from_option() {
        let expected_pagesize = 2048;
        let expected_size = 107374182;
        let expected = (expected_pagesize, expected_size);

        let data = vec![
            // (input, expected, is_ok)
            ("size-1=107374182,pagesize-1=2048", expected, false),
            ("size-1=107374182,pagesize=2048", expected, false),
            ("size=107374182,pagesize-1=2048", expected, false),
            ("size=107374182,pagesize=abc", expected, false),
            ("size=abc,pagesize=2048", expected, false),
            ("size=,pagesize=2048", expected, false),
            ("size=107374182,pagesize=", expected, false),
            ("size=107374182,pagesize=2048", expected, true),
            ("pagesize=2048,size=107374182", expected, true),
            ("foo=bar,pagesize=2048,size=107374182", expected, true),
            (
                "foo=bar,pagesize=2048,foo1=bar1,size=107374182",
                expected,
                true,
            ),
            (
                "pagesize=2048,foo1=bar1,foo=bar,size=107374182",
                expected,
                true,
            ),
            (
                "foo=bar,pagesize=2048,foo1=bar1,size=107374182,foo2=bar2",
                expected,
                true,
            ),
            (
                "foo=bar,size=107374182,foo1=bar1,pagesize=2048",
                expected,
                true,
            ),
        ];

        for case in data {
            let input = case.0;
            let r = EphemeralHandler::get_pagesize_and_size_from_option(&[input.to_string()]);

            let is_ok = case.2;
            if is_ok {
                let expected = case.1;
                let (pagesize, size) = r.unwrap();
                assert_eq!(expected.0, pagesize);
                assert_eq!(expected.1, size);
            } else {
                assert!(r.is_err());
            }
        }
    }

    #[test]
    fn test_allocate_hugepages_sums_containers() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let sysfs = tempfile::tempdir().unwrap();
        let dir = sysfs.path().join("hugepages-2048kB");
        fs::create_dir_all(&dir).unwrap();
        let nr = dir.join("nr_hugepages");
        fs::write(&nr, "4").unwrap();
        let read = || fs::read_to_string(&nr).unwrap();
        let opts = |size: u64| vec![format!("pagesize=2097152,size={}", size)];
        let cid = |id: &str| Some(format!("{}-{}", id, std::process::id()));
        let allocate = |size, cid: &Option<String>| {
            EphemeralHandler::allocate_hugepages(&logger, &opts(size), cid, sysfs.path()).unwrap()
        };

        assert!(allocate(2 << 20, &cid("a")).is_some());
        assert_eq!(read(), "5");
        assert!(allocate(2 << 20, &cid("a")).is_none());
        assert_eq!(read(), "5");
        let b = allocate(6 << 20, &cid("b")).unwrap();
        assert_eq!(read(), "8");

        b.release().unwrap();
        assert_eq!(read(), "5");
        assert!(allocate(6 << 20, &cid("b")).is_some());
        assert_eq!(read(), "8");
    }
}
