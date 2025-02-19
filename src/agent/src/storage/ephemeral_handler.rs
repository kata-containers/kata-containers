// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
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
            Self::allocate_hugepages(ctx.logger, &storage.options.to_vec())
                .context("allocate hugepages")?;
            common_storage_handler(ctx.logger, &storage)?;
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
    // Allocate hugepages by writing to sysfs
    fn allocate_hugepages(logger: &Logger, options: &[String]) -> Result<()> {
        info!(logger, "mounting hugePages storage options: {:?}", options);

        let (pagesize, size) = Self::get_pagesize_and_size_from_option(options)
            .context(format!("parse mount options: {:?}", &options))?;

        info!(
            logger,
            "allocate hugepages. pageSize: {}, size: {}", pagesize, size
        );

        // sysfs entry is always of the form hugepages-${pagesize}kB
        // Ref: https://www.kernel.org/doc/Documentation/vm/hugetlbpage.txt
        let path = Path::new(SYS_FS_HUGEPAGES_PREFIX)
            .join(format!("hugepages-{}kB", pagesize / 1024))
            .join("nr_hugepages");

        // write numpages to nr_hugepages file.
        let numpages = format!("{}", size / pagesize);
        info!(logger, "write {} pages to {:?}", &numpages, &path);

        let mut file = OpenOptions::new()
            .write(true)
            .open(&path)
            .context(format!("open nr_hugepages directory {:?}", &path))?;

        file.write_all(numpages.as_bytes())
            .context(format!("write nr_hugepages failed: {:?}", &path))?;

        // Even if the write succeeds, the kernel isn't guaranteed to be
        // able to allocate all the pages we requested.  Verify that it
        // did.
        let verify = fs::read_to_string(&path).context(format!("reading {:?}", &path))?;
        let allocated = verify
            .trim_end()
            .parse::<u64>()
            .map_err(|_| anyhow!("Unexpected text {:?} in {:?}", &verify, &path))?;
        if allocated != size / pagesize {
            return Err(anyhow!(
                "Only allocated {} of {} hugepages of size {}",
                allocated,
                numpages,
                pagesize
            ));
        }

        Ok(())
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
            .context(format!("parse pagesize: {:?}", &pagesize_str))?;
        let size = size_str
            .unwrap()
            .parse::<u64>()
            .context(format!("parse size: {:?}", &size_str))?;

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
}
