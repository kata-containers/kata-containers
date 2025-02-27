// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{io, mem};

use anyhow::{Context, Result};
use nix::sys::socket::{socket, AddressFamily, SockFlag, SockType};
use scopeguard::defer;

use super::macros::{get_name, set_name};

/// FW version length
const ETHTOOL_FW_VERSION_LEN: usize = 32;

/// bus info length
const ETHTOOL_BUS_INFO_LEN: usize = 32;

/// erom version length
const ETHTOOL_EROM_VERSION_LEN: usize = 32;

/// driver info
const ETHTOOL_DRIVER_INFO: u32 = 0x00000003;

/// Ethtool interface define 0x8946
const IOCTL_ETHTOOL_INTERFACE: u32 = 0x8946;

nix::ioctl_readwrite_bad!(ioctl_ethtool, IOCTL_ETHTOOL_INTERFACE, DeviceInfoReq);

#[repr(C)]
pub union DeviceInfoIfru {
    pub ifr_addr: libc::sockaddr,
    pub ifr_data: *mut libc::c_char,
}

type IfName = [u8; libc::IFNAMSIZ];

#[repr(C)]
pub struct DeviceInfoReq {
    pub ifr_name: IfName,
    pub ifr_ifru: DeviceInfoIfru,
}

impl DeviceInfoReq {
    pub fn from_name(name: &str) -> io::Result<DeviceInfoReq> {
        let mut req: DeviceInfoReq = unsafe { mem::zeroed() };
        req.set_name(name)?;
        Ok(req)
    }

    pub fn set_name(&mut self, name: &str) -> io::Result<()> {
        set_name!(self.ifr_name, name)
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
struct Driver {
    pub cmd: u32,
    pub driver: [u8; 32],
    pub version: [u8; 32],
    pub fw_version: [u8; ETHTOOL_FW_VERSION_LEN],
    pub bus_info: [u8; ETHTOOL_BUS_INFO_LEN],
    pub erom_version: [u8; ETHTOOL_EROM_VERSION_LEN],
    pub reserved2: [u8; 12],
    pub n_priv_flags: u32,
    pub n_stats: u32,
    pub test_info_len: u32,
    pub eedump_len: u32,
    pub regdump_len: u32,
}

#[derive(Debug, Clone)]
pub struct DriverInfo {
    #[allow(dead_code)]
    pub driver: String,
    pub bus_info: String,
}

pub fn get_driver_info(name: &str) -> Result<DriverInfo> {
    let mut req = DeviceInfoReq::from_name(name).context(format!("ifreq from name {}", name))?;
    let mut ereq: Driver = unsafe { mem::zeroed() };
    ereq.cmd = ETHTOOL_DRIVER_INFO;
    req.ifr_ifru.ifr_data = &mut ereq as *mut _ as *mut _;

    let fd = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .context("new socket")?;
    defer!({
        let _ = nix::unistd::close(fd);
    });
    unsafe { ioctl_ethtool(fd, &mut req).context("ioctl ethtool")? };
    Ok(DriverInfo {
        driver: get_name!(ereq.driver).context("get driver name")?,
        bus_info: get_name!(ereq.bus_info).context("get bus info name")?,
    })
}
