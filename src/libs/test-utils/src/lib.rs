// Copyright (c) 2019 Intel Corporation
// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[derive(Debug, PartialEq)]
pub enum TestUserType {
    RootOnly,
    NonRootOnly,
    Any,
}

#[macro_export]
macro_rules! skip_if_root {
    () => {
        if nix::unistd::Uid::effective().is_root() {
            println!("INFO: skipping {} which needs non-root", module_path!());
            return;
        }
    };
}

#[macro_export]
macro_rules! skip_if_not_root {
    () => {
        if !nix::unistd::Uid::effective().is_root() {
            println!("INFO: skipping {} which needs root", module_path!());
            return;
        }
    };
}

#[macro_export]
macro_rules! skip_loop_if_root {
    ($msg:expr) => {
        if nix::unistd::Uid::effective().is_root() {
            println!(
                "INFO: skipping loop {} in {} which needs non-root",
                $msg,
                module_path!()
            );
            continue;
        }
    };
}

#[macro_export]
macro_rules! skip_loop_if_not_root {
    ($msg:expr) => {
        if !nix::unistd::Uid::effective().is_root() {
            println!(
                "INFO: skipping loop {} in {} which needs root",
                $msg,
                module_path!()
            );
            continue;
        }
    };
}

// Parameters:
//
// 1: expected Result
// 2: actual Result
// 3: string used to identify the test on error
#[macro_export]
macro_rules! assert_result {
    ($expected_result:expr, $actual_result:expr, $msg:expr) => {
        if $expected_result.is_ok() {
            let expected_value = $expected_result.as_ref().unwrap();
            let actual_value = $actual_result.unwrap();
            assert!(*expected_value == actual_value, "{}", $msg);
        } else {
            assert!($actual_result.is_err(), "{}", $msg);

            let expected_error = $expected_result.as_ref().unwrap_err();
            let expected_error_msg = format!("{:?}", expected_error);

            let actual_error_msg = format!("{:?}", $actual_result.unwrap_err());

            assert!(expected_error_msg == actual_error_msg, "{}", $msg);
        }
    };
}

#[macro_export]
macro_rules! skip_loop_by_user {
    ($msg:expr, $user:expr) => {
        if $user == TestUserType::RootOnly {
            skip_loop_if_not_root!($msg);
        } else if $user == TestUserType::NonRootOnly {
            skip_loop_if_root!($msg);
        }
    };
}

#[macro_export]
macro_rules! skip_if_rng_ioctl_restricted {
    () => {
        // Try the exact same ioctl operations as reseed_rng() to detect restrictions
        use nix::errno::Errno;
        use nix::fcntl::{self, OFlag};
        use nix::sys::stat::Mode;
        use std::os::unix::io::AsRawFd;

        // Use architecture-specific ioctl values (same as random.rs)
        #[cfg(all(target_arch = "powerpc64", target_endian = "little"))]
        const RNDADDTOENTCNT: libc::c_uint = 0x80045201;
        #[cfg(all(target_arch = "powerpc64", target_endian = "little"))]
        const RNDRESEEDCRNG: libc::c_int = 0x20005207;
        #[cfg(not(target_arch = "powerpc64"))]
        const RNDADDTOENTCNT: libc::c_int = 0x40045201;
        #[cfg(not(target_arch = "powerpc64"))]
        const RNDRESEEDCRNG: libc::c_int = 0x5207;

        // Handle the differing ioctl(2) request types for different targets (same as random.rs)
        #[cfg(target_env = "musl")]
        type IoctlRequestType = libc::c_int;
        #[cfg(target_env = "gnu")]
        type IoctlRequestType = libc::c_ulong;

        match fcntl::open("/dev/random", OFlag::O_RDWR, Mode::from_bits_truncate(0o022)) {
            Ok(fd) => {
                let len: libc::c_long = 1; // Use minimal length for test

                // Try RNDADDTOENTCNT ioctl (same as reseed_rng)
                let ret = unsafe {
                    libc::ioctl(
                        fd,
                        RNDADDTOENTCNT as IoctlRequestType,
                        &len as *const libc::c_long,
                    )
                };

                if let Err(e) = Errno::result(ret).map(drop) {
                    let _ = nix::unistd::close(fd);
                    if e == Errno::EPERM {
                        println!(
                            "INFO: skipping {} - RNG ioctls are restricted in this environment (EPERM)",
                            module_path!()
                        );
                        return;
                    }
                }

                // Try RNDRESEEDCRNG ioctl (same as reseed_rng)
                let ret = unsafe { libc::ioctl(fd, RNDRESEEDCRNG as IoctlRequestType, 0) };
                let _ = nix::unistd::close(fd);

                if let Err(e) = Errno::result(ret).map(drop) {
                    if e == Errno::EPERM {
                        println!(
                            "INFO: skipping {} - RNG ioctls are restricted in this environment (EPERM)",
                            module_path!()
                        );
                        return;
                    }
                }
            }
            Err(_) => {
                println!(
                    "INFO: skipping {} - cannot open /dev/random for RNG ioctl test",
                    module_path!()
                );
                return;
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{skip_if_not_root, skip_if_root};

    #[test]
    fn test_skip_if_not_root() {
        skip_if_not_root!();
        assert!(
            nix::unistd::Uid::effective().is_root(),
            "normal user should be skipped"
        )
    }

    #[test]
    fn test_skip_if_root() {
        skip_if_root!();
        assert!(
            !nix::unistd::Uid::effective().is_root(),
            "root user should be skipped"
        )
    }
}
