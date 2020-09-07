// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libc;
use nix::errno::Errno;
use nix::fcntl::{self, OFlag};
use nix::sys::stat::Mode;
use std::fs;

pub const RNGDEV: &str = "/dev/random";
pub const RNDADDTOENTCNT: libc::c_int = 0x40045201;
pub const RNDRESEEDRNG: libc::c_int = 0x5207;

// Handle the differing ioctl(2) request types for different targets
#[cfg(target_env = "musl")]
type IoctlRequestType = libc::c_int;
#[cfg(target_env = "gnu")]
type IoctlRequestType = libc::c_ulong;

pub fn reseed_rng(data: &[u8]) -> Result<()> {
    let len = data.len() as libc::c_long;
    fs::write(RNGDEV, data)?;

    let fd = fcntl::open(RNGDEV, OFlag::O_RDWR, Mode::from_bits_truncate(0o022))?;

    let ret = unsafe {
        libc::ioctl(
            fd,
            RNDADDTOENTCNT as IoctlRequestType,
            &len as *const libc::c_long,
        )
    };
    let _ = Errno::result(ret).map(drop)?;

    let ret = unsafe { libc::ioctl(fd, RNDRESEEDRNG as IoctlRequestType, 0) };
    let _ = Errno::result(ret).map(drop)?;

    Ok(())
}
