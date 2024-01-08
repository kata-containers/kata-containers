// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use ini::Ini;

const SYS_DEV_PREFIX: &str = "/sys/dev";

pub const DEVICE_TYPE_BLOCK: &str = "b";
pub const DEVICE_TYPE_CHAR: &str = "c";

// get_host_path is used to fetch the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
pub fn get_host_path(dev_type: &str, major: i64, minor: i64) -> Result<String> {
    let path_comp = match dev_type {
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

// Do increase attach reference count when the device is attached.
pub fn do_increase_count(ref_count: &mut u64) -> Result<bool> {
    match *ref_count {
        0 => {
            // When ref_count is 0, it indicates that the device is new here。
            // In this case, we know that the device has not been attached into the Guest.
            *ref_count += 1;
            Ok(false)
        }
        // While in practice, the number of times a device is attempted to be inserted into the Guest
        // cannot reach this maximum value, the device is theoretically allowed to do so.
        // However, we will not allow the device to actually be inserted into the Guest more than once.
        std::u64::MAX => Err(anyhow!("This device was attached too many times")),
        _ => {
            // When ref_count is greater than 0, it indicates that how many times the device has been
            // attempted to be inserted into the Guest.
            // In this case, we know the device has been attached into the Guest.
            *ref_count += 1;
            Ok(true)
        }
    }
}

// Do decrease attach reference count when the device is detached.
pub fn do_decrease_count(ref_count: &mut u64) -> Result<bool> {
    match *ref_count {
        // When ref_count is 0, it indicates that the device is not inserted into the Guest,
        // it's not allowed to decrease count.
        0 => Err(anyhow!("The device is not attached")),
        1 => {
            // When ref_count is 1, it indicates that the device has been inserted into the Guest.
            // In this case, we know the ref count can be decreased.
            // And let the Device Manager know that the device is detached from the Guest.
            *ref_count -= 1;
            Ok(false)
        }
        _ => {
            // When the ref count greater than 1, it just decreases the ref count.
            // And let the Device Manager know that the device is still still inserted in the Guest.
            *ref_count -= 1;
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::device::util::get_virt_drive_name;
    use crate::device::util::{do_decrease_count, do_increase_count};

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

    #[test]
    fn test_do_increase_count() {
        // First, ref_count is 0
        let ref_count_0 = &mut 0_u64;
        let _ = do_decrease_count(ref_count_0).is_err();

        assert!(!do_increase_count(ref_count_0).unwrap());
        assert!(!do_decrease_count(ref_count_0).unwrap());

        // Second, ref_count > 0
        let ref_count_3 = &mut 3_u64;
        assert!(do_increase_count(ref_count_3).unwrap());
        assert!(do_decrease_count(ref_count_3).unwrap());

        // Third, ref_count is MAX
        let mut max_count = std::u64::MAX;
        let ref_count_max: &mut u64 = &mut max_count;
        let _ = do_increase_count(ref_count_max).is_err();
    }

    #[test]
    fn test_data_reference_count() {
        #[derive(Default)]
        struct TestData {
            ref_cnt: u64,
        }

        impl TestData {
            fn attach(&mut self) -> bool {
                do_increase_count(&mut self.ref_cnt).unwrap()
            }

            fn detach(&mut self) -> bool {
                do_decrease_count(&mut self.ref_cnt).unwrap()
            }
        }

        let testd = &mut TestData { ref_cnt: 0_u64 };

        // First, ref_cnt is 0
        assert!(!testd.attach());
        assert_eq!(testd.ref_cnt, 1_u64);
        // Second, ref_cnt > 0
        assert!(testd.attach());
        assert_eq!(testd.ref_cnt, 2_u64);
        assert!(testd.attach());
        assert_eq!(testd.ref_cnt, 3_u64);

        let testd2 = &mut TestData { ref_cnt: 2_u64 };

        assert!(testd2.detach());
        assert_eq!(testd2.ref_cnt, 1_u64);

        assert!(!testd2.detach());
        assert_eq!(testd2.ref_cnt, 0_u64);
    }
}
