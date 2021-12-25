// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::io::Result;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

use nix::sys::stat;

use crate::{eother, sl};

const SYS_DEV_BLOCK_PATH: &str = "/sys/dev/block";
const BLKDEV_PARTITION: &str = "partition";
const BLKDEV_DEV_FILE: &str = "dev";

/// Get major and minor number of the device or of the device hosting the regular file/directory.
pub fn get_devid_for_blkio_cgroup<P: AsRef<Path>>(path: P) -> Result<Option<(u64, u64)>> {
    let md = fs::metadata(path)?;

    if md.is_dir() || md.is_file() {
        // For regular file/directory, get major/minor of the block device hosting it.
        // Note that we need to get the major/minor of the block device instead of partition,
        // e.g. /dev/sda instead of /dev/sda3, because blkio cgroup works with block major/minor.
        let id = md.dev();
        Ok(Some((stat::major(id), stat::minor(id))))
    } else if md.file_type().is_block_device() {
        // For block device, get major/minor of the device special file itself
        get_block_device_id(md.rdev())
    } else {
        Ok(None)
    }
}

/// Get the block device major/minor number from a partition/block device(itself).
///
/// For example, given the dev_t of /dev/sda3 returns major and minor of /dev/sda. We rely on the
/// fact that if /sys/dev/block/$major:$minor/partition exists, then it's a partition, and find its
/// parent for the real device.
fn get_block_device_id(dev: stat::dev_t) -> Result<Option<(u64, u64)>> {
    let major = stat::major(dev);
    let minor = stat::minor(dev);
    let mut blk_dev_path = PathBuf::from(SYS_DEV_BLOCK_PATH)
        .join(format!("{}:{}", major, minor))
        .canonicalize()?;

    // If 'partition' file exists, then it's a partition of the real device, take its parent.
    // Otherwise it's already the real device.
    loop {
        if !blk_dev_path.join(BLKDEV_PARTITION).exists() {
            break;
        }
        blk_dev_path = match blk_dev_path.parent() {
            Some(p) => p.to_path_buf(),
            None => {
                return Err(eother!(
                    "Can't find real device for dev {}:{}",
                    major,
                    minor
                ))
            }
        };
    }

    // Parse major:minor in dev file
    let dev_path = blk_dev_path.join(BLKDEV_DEV_FILE);
    let dev_buf = fs::read_to_string(&dev_path)?;
    let dev_buf = dev_buf.trim_end();
    debug!(
        sl!(),
        "get_real_devid: dev {}:{} -> {:?} ({})", major, minor, blk_dev_path, dev_buf
    );

    if let Some((major, minor)) = dev_buf.split_once(':') {
        let major = major
            .parse::<u64>()
            .map_err(|_e| eother!("Failed to parse major number: {}", major))?;
        let minor = minor
            .parse::<u64>()
            .map_err(|_e| eother!("Failed to parse minor number: {}", minor))?;
        Ok(Some((major, minor)))
    } else {
        Err(eother!(
            "Wrong format in {}: {}",
            dev_path.to_string_lossy(),
            dev_buf
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_devid() {
        //let (major, minor) = get_devid_for_blkio_cgroup("/dev/vda1").unwrap().unwrap();
        assert!(get_devid_for_blkio_cgroup("/dev/tty").unwrap().is_none());
        get_devid_for_blkio_cgroup("/do/not/exist/file_______name").unwrap_err();
    }
}
