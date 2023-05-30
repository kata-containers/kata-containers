// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use ini::Ini;

const SYS_DEV_PREFIX: &str = "/sys/dev";

// get_host_path is used to fetch the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
pub fn get_host_path(dev_type: String, major: i64, minor: i64) -> Result<String> {
    let path_comp = match dev_type.as_str() {
        "c" | "u" => "char",
        "b" => "block",
        // for device type p will return an empty string
        _ => return Ok(String::new()),
    };
    let format = format!("{}:{}", major, minor);
    let sys_dev_path = std::path::Path::new(SYS_DEV_PREFIX)
        .join(path_comp)
        .join(format)
        .join("uevent");
    std::fs::metadata(&sys_dev_path)?;
    let conf = Ini::load_from_file(&sys_dev_path)?;
    let dev_name = conf
        .section::<String>(None)
        .ok_or_else(|| anyhow!("has no section"))?
        .get("DEVNAME")
        .ok_or_else(|| anyhow!("has no DEVNAME"))?;
    Ok(format!("/dev/{}", dev_name))
}

// get_virt_drive_name returns the disk name format for virtio-blk
// Reference: https://github.com/torvalds/linux/blob/master/drivers/block/virtio_blk.c @c0aa3e0916d7e531e69b02e426f7162dfb1c6c0
pub(crate) fn get_virt_drive_name(mut index: i32) -> Result<String> {
    if index < 0 {
        return Err(anyhow!("Index cannot be negative"));
    }

    // Prefix used for virtio-block devices
    const PREFIX: &str = "vd";

    // Refer to DISK_NAME_LEN: https://github.com/torvalds/linux/blob/08c521a2011ff492490aa9ed6cc574be4235ce2b/include/linux/genhd.h#L61
    let disk_name_len = 32usize;
    let base = 26i32;

    let suff_len = disk_name_len - PREFIX.len();
    let mut disk_letters = vec![0u8; suff_len];

    let mut i = 0usize;
    while i < suff_len && index >= 0 {
        let letter: u8 = b'a' + (index % base) as u8;
        disk_letters[i] = letter;
        index = (index / base) - 1;
        i += 1;
    }
    if index >= 0 {
        return Err(anyhow!("Index not supported"));
    }
    disk_letters.truncate(i);
    disk_letters.reverse();
    Ok(String::from(PREFIX) + std::str::from_utf8(&disk_letters)?)
}

#[cfg(test)]
mod tests {
    use crate::device::util::get_virt_drive_name;

    #[actix_rt::test]
    async fn test_get_virt_drive_name() {
        for &(input, output) in [
            (0i32, "vda"),
            (25, "vdz"),
            (27, "vdab"),
            (704, "vdaac"),
            (18277, "vdzzz"),
        ]
        .iter()
        {
            let out = get_virt_drive_name(input).unwrap();
            assert_eq!(&out, output);
        }
    }
}
