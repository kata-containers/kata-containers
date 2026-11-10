// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{io::ErrorKind, path::Path};

use anyhow::{anyhow, Result};
use ini::Ini;

const SYS_DEV_PREFIX: &str = "/sys/dev";

pub const DEVICE_TYPE_BLOCK: &str = "b";
pub const DEVICE_TYPE_CHAR: &str = "c";

// get_host_path is used to fetch the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
//
// The `fallback_path` argument provides a fallback for cases where there's no
// /sys/dev for the major:minor combination given.  This is not an issue with
// block devices: a block device's /dev node and its /sys/dev entry both come
// from the same registration, so in practice a block node always has a sysfs
// entry.  Callers resolving a block major:minor can therefore just pass None.
//
// With character devices however it can well happen that they have no
// /sys/dev entry - for instance if their /dev file is created statically but
// the corresponding driver is not loaded in the kernel (often the case with
// /dev/fuse or /dev/cuse), or the driver does not register its devices under
// /sys/dev/char like the pseudoterminal driver.  For the runtime to provide a
// best-effort service in those cases, char-dev callers should pass the device
// path from the OCI spec device config block as a fallback.  Note that device
// renaming thus cannot be supported in those cases.  While the device might
// ultimately not work if actually accessed from the container, it does help
// privileged containers which - depending on container engine configuration -
// can get all host devices, even those they don't actually care about.
pub fn get_host_path(
    dev_type: &str,
    major: i64,
    minor: i64,
    fallback_path: Option<&Path>,
) -> Result<String> {
    let path_comp = match dev_type {
        "c" | "u" => "char",
        "b" => "block",
        // for device type p will return an empty string
        _ => return Ok(String::new()),
    };
    let format = format!("{major}:{minor}");
    let sys_dev_path = std::path::Path::new(SYS_DEV_PREFIX)
        .join(path_comp)
        .join(format)
        .join("uevent");

    if let Err(err) = std::fs::metadata(&sys_dev_path) {
        if err.kind() != ErrorKind::NotFound {
            return Err(err.into());
        }
        let fallback_path = fallback_path.ok_or_else(|| {
            anyhow!(
                "no {sys_dev_path:?} path for device {major}:{minor} and no fallback path given"
            )
        })?;
        return fallback_path
            .to_str()
            .ok_or_else(|| anyhow!("fallback path {fallback_path:?} is not valid UTF-8"))
            .map(str::to_owned);
    }

    let conf = Ini::load_from_file(&sys_dev_path)?;
    let dev_name = conf
        .section::<String>(None)
        .ok_or_else(|| anyhow!("has no section"))?
        .get("DEVNAME")
        .ok_or_else(|| anyhow!("has no DEVNAME"))?;
    Ok(format!("/dev/{dev_name}"))
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

// Using the return value of do_increase_count to indicate whether a device has been inserted into the guest.
// Specially, Increment the reference count by 1, then check the incremented ref_count:
// If the incremented reference count is not equal to 1, the device has been inserted into the guest. Return true.
// If the reference count is equal to 1, the device has not been inserted into the guest. Return false.
pub fn do_increase_count(ref_count: &mut u64) -> Result<bool> {
    // ref_count = 0: Device is new and not attached.
    // ref_count > 0: Device has been attempted to be attached many times.
    *ref_count = (*ref_count)
        .checked_add(1)
        .ok_or("device reference count overflow")
        .map_err(|e| anyhow!(e))?;

    Ok((*ref_count) != 1)
}

// The return value of do_decrease_count can be used to indicate whether the device is still in use.
// Specifically, the reference count can be decremented by 1 first, then check the decremented ref_count:
// If the decremented reference count is not equal to 0, it indicates that the device is still in use by
// the guest and cannot be detached. Return true.
// If the reference count is equal to 0, it indicates that the device will not be used and can be unplugged
// from the guest. Return false.
pub fn do_decrease_count(ref_count: &mut u64) -> Result<bool> {
    // ref_count = 0: Device not inserted (cannot decrease further).
    // ref_count = 1: Device is attached to the Guest. Decrement ref_count and notify Device Manager of detachment.
    // ref_count > 1: Device remains attached to the Guest. Simply decrement ref_count and notify Device Manager.
    *ref_count = (*ref_count)
        .checked_sub(1)
        .ok_or("The device is not attached")
        .map_err(|e| anyhow!(e))?;

    Ok((*ref_count) != 0)
}

#[cfg(test)]
mod tests {
    use crate::device::util::{do_decrease_count, do_increase_count};
    use crate::device::util::{
        get_host_path, get_virt_drive_name, DEVICE_TYPE_BLOCK, DEVICE_TYPE_CHAR,
    };
    use std::path::Path;

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
        assert!(do_decrease_count(ref_count_0).is_err());

        assert!(!do_increase_count(ref_count_0).unwrap());
        assert!(!do_decrease_count(ref_count_0).unwrap());

        // Second, ref_count > 0
        let ref_count_3 = &mut 3_u64;
        assert!(do_increase_count(ref_count_3).unwrap());
        assert!(do_decrease_count(ref_count_3).unwrap());

        // Third, ref_count is MAX
        let mut max_count = u64::MAX;
        let ref_count_max: &mut u64 = &mut max_count;
        assert!(do_increase_count(ref_count_max).is_err());
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

    #[test]
    fn test_get_host_path() {
        // device with no /sys/dev entry, with a fallback
        // we assume here that 4095:4095 is unallocated
        assert_eq!(
            get_host_path(DEVICE_TYPE_CHAR, 4095, 4095, Some(Path::new("/dev/cuse"))).unwrap(),
            "/dev/cuse"
        );
        // device with no /sys/dev entry and no fallback
        assert!(get_host_path(DEVICE_TYPE_CHAR, 4095, 4095, None).is_err());
        // device with /sys/dev entry, with a fallback
        // we assume that /dev/null exists
        assert_eq!(
            get_host_path(DEVICE_TYPE_CHAR, 1, 3, Some(Path::new("/dev/wrong"))).unwrap(),
            "/dev/null"
        );
        // we assume that 99:99 is unallocated
        assert!(get_host_path(DEVICE_TYPE_BLOCK, 99, 99, None).is_err());
    }
}
