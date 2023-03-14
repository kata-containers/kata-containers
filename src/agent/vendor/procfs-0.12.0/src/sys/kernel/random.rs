//! These files provide additional information about the /dev/random device
//!
//! Note that some of these entries are only documented in random(4), while some are also documented under proc(5)

use crate::{read_value, write_value, ProcError, ProcResult};
use lazy_static::lazy_static;

lazy_static! {
    static ref RANDOM_ROOT: std::path::PathBuf = std::path::PathBuf::from("/proc/sys/kernel/random");
}

/// This read-only file gives the available entropy, in bits. This will be a number in the range
/// 0 to 4096
pub fn entropy_avail() -> ProcResult<u16> {
    read_value(RANDOM_ROOT.join("entropy_avail"))
}

/// This file gives the size of the entropy pool
///
/// The semantics of this file are different on kernel versions older than 2.6, however, since
/// Linux 2.6 it is read-only, and gives the size of the entropy pool in bits, containing the value 4096.
///
/// See `man random(4)` for more information
pub fn poolsize() -> ProcResult<u16> {
    read_value(RANDOM_ROOT.join("poolsize"))
}

/// This file contains the number of bits of entropy required for waking up processes that sleep waiting
/// for entropy from /dev/random
///
/// The default is 64.
///
/// This will first attempt to read from `/proc/sys/kernel/random/read_wakeup_threshold` but it
/// will fallback to `/proc/sys/kernel/random/write_wakeup_threshold` if the former file is not found.
pub fn read_wakeup_threshold() -> ProcResult<u32> {
    match read_value(RANDOM_ROOT.join("read_wakeup_threshold")) {
        Ok(val) => Ok(val),
        Err(err) => match err {
            ProcError::NotFound(_) => read_value(RANDOM_ROOT.join("write_wakeup_threshold")),
            err => Err(err),
        },
    }
}

/// This file contains the number of bits of entropy below which we wake up processes that do a
/// select(2) or poll(2) for write access to /dev/random. These values can be changed by writing to the file.
pub fn write_wakeup_threshold(new_value: u32) -> ProcResult<()> {
    write_value(RANDOM_ROOT.join("write_wakeup_threshold"), new_value)
}

/// This read-only file randomly generates a fresh 128-bit UUID on each read
pub fn uuid() -> ProcResult<String> {
    read_value(RANDOM_ROOT.join("uuid"))
}

/// This is a read-only file containing a 128-bit UUID generated at boot
pub fn boot_id() -> ProcResult<String> {
    read_value(RANDOM_ROOT.join("boot_id"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_avail() {
        let entropy = entropy_avail().unwrap();
        assert!(entropy <= 4096);
    }

    #[test]
    fn test_poolsize() {
        // The kernel support section in the root lib.rs file says that we only aim to support >= 2.6 kernels,
        // so only test that case
        let poolsize = poolsize().unwrap();
        assert!(poolsize == 4096)
    }

    #[test]
    fn test_read_wakeup_threshold() {
        let threshold = read_wakeup_threshold().unwrap();

        println!("{}", threshold);
    }

    #[test]
    fn test_write_wakeup_threshold() {
        let old_threshold = read_wakeup_threshold().unwrap();

        match write_wakeup_threshold(1024) {
            Ok(_) => (),
            Err(err) => match err {
                ProcError::PermissionDenied(_) => {
                    // This is ok, not everyone wants to run our tests as root
                    return;
                }
                err => panic!("test_write_wakeup_threshold error: {:?}", err),
            },
        }

        // If we got here, let's restore the old threshold
        let _ = write_wakeup_threshold(old_threshold);
    }

    #[test]
    fn test_uuid_fns() {
        let uuid = uuid().unwrap();
        let boot_id = boot_id().unwrap();

        println!("UUID: {}", uuid);
        println!("boot UUID: {}", boot_id);
    }
}
