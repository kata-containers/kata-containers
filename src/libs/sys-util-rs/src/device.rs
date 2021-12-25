// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::io::Result;
use std::path::PathBuf;

use nix::sys::stat::{self, SFlag};

use crate::{eother, sl};

const SYS_DEV_BLOCK_PATH: &str = "/sys/dev/block";

/// Get major and minor number of the device or of the device hosting the regular file/directory.
pub fn get_devid(file: &str) -> Result<(u64, u64)> {
    let fstat =
        stat::stat(file).map_err(|e| eother!("get_devid: failed to stat {}: {:?}", file, e))?;

    match SFlag::from_bits_truncate(fstat.st_mode) {
        // For block device, check major/minor of the device special file itself
        SFlag::S_IFBLK => Ok((
            stat::major(fstat.st_rdev) as u64,
            stat::minor(fstat.st_rdev) as u64,
        )),
        // For regular file, check major/minor of the underlying block device. Note that we need to
        // get the major/minor of the real device not partition, e.g. /dev/sda not /dev/sda3
        //
        // We also accept dir here, as sandbox builtin storage is unlinked after open, and we only
        // care about the underlying fs and block device, not the image file itself.
        SFlag::S_IFREG | SFlag::S_IFDIR => get_real_devid(fstat.st_dev),
        // Not block device, nor regular file, caller should handle this case.
        _ => Ok((0, 0)),
    }
}

/// Get the real device major/minor number for the device.
///
/// For example, given the dev_t of /dev/sda3 returns major and minor of /dev/sda. We rely on the
/// fact that if /sys/dev/block/$major:$minor/partition exists, then it's a partition, and find its
/// parent for the real device.
fn get_real_devid(dev: nix::sys::stat::dev_t) -> Result<(u64, u64)> {
    let major = stat::major(dev);
    let minor = stat::minor(dev);
    let mut real_dev_path = PathBuf::from(SYS_DEV_BLOCK_PATH)
        .join(format!("{}:{}", major, minor))
        .canonicalize()?;

    // If 'partition' file exists, then it's a partition of the real device, take its parent.
    // Otherwise it's already the real device.
    loop {
        if real_dev_path.join("partition").exists() {
            real_dev_path = match real_dev_path.parent() {
                Some(p) => p.to_path_buf(),
                None => {
                    return Err(eother!(
                        "Can't find real device for dev {}:{}",
                        major,
                        minor
                    ))
                }
            };
            continue;
        } else {
            break;
        }
    }

    // Parse major:minor in dev file
    let dev_path = real_dev_path.join("dev");
    let dev_buf = fs::read_to_string(&dev_path)?;
    let dev_buf = dev_buf.trim_end();
    debug!(
        sl!(),
        "get_real_devid: dev {}:{} -> {:?} ({})", major, minor, real_dev_path, dev_buf
    );

    let str_vec: Vec<&str> = dev_buf.split(':').collect();
    match str_vec.len() {
        2 => {
            let major = str_vec[0]
                .parse::<u64>()
                .map_err(|_e| eother!("Failed to parse major number: {}", str_vec[0]))?;
            let minor = str_vec[1]
                .parse::<u64>()
                .map_err(|_e| eother!("Failed to parse minor number: {}", str_vec[1]))?;
            Ok((major, minor))
        }
        _ => {
            return Err(eother!(
                "Wrong format in {}: {}",
                dev_path.to_string_lossy(),
                dev_buf
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_devid() {
        get_devid("/proc/mounts").unwrap_err();
        get_devid("/do/not/exist/file_______name").unwrap_err();
    }
}
