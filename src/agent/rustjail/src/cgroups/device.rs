// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use cgroups::Manager;
use oci_spec::runtime::{
    LinuxDeviceCgroup, LinuxDeviceCgroupBuilder, LinuxDeviceType, LinuxResourcesBuilder, Spec,
};

use crate::container::DEFAULT_DEVICES;

pub fn new_device_cgroup(
    allow: bool,
    dev_type: Option<LinuxDeviceType>,
    major: Option<i64>,
    minor: Option<i64>,
    access: Option<&str>,
) -> Result<LinuxDeviceCgroup> {
    let mut builder = LinuxDeviceCgroupBuilder::default().allow(allow);

    if let Some(typ) = dev_type {
        builder = builder.typ(typ);
    }

    if let Some(maj) = major {
        builder = builder.major(maj);
    }

    if let Some(min) = minor {
        builder = builder.minor(min);
    }

    if let Some(acc) = access {
        builder = builder.access(acc.to_string());
    }

    builder.build().context("build LinuxDeviceCgroup")
}

/// Check if the linux device cgroup grants universal access (rwm) to all
/// devices.
///
/// The formats representing all devices between OCI spec and cgroups-rs
/// are different.
/// - OCI spec: major: Some(0), minor: Some(0), type: Some(A), access: Some("rwm");
/// - Cgroups-rs: major: -1, minor: -1, type: "a", access: "rwm";
/// - Linux: a *:* rwm
pub fn is_linux_devcg_allowed_all(dev: &LinuxDeviceCgroup) -> bool {
    let cgrp_access = dev.access().clone().unwrap_or_default();
    let dev_type = dev
        .typ()
        .as_ref()
        .map_or(LinuxDeviceType::default(), |x| *x);
    dev.major().unwrap_or(0) == 0
        && dev.minor().unwrap_or(0) == 0
        && dev_type == LinuxDeviceType::A
        && cgrp_access.contains('r')
        && cgrp_access.contains('w')
        && cgrp_access.contains('m')
}

/// Check if OCI spec grants universal access (rwm) to all devices.
pub fn has_oci_spec_allowed_all(spec: &Spec) -> bool {
    // spec.linux()
    spec.linux()
        .as_ref()
        .and_then(|l| l.resources().as_ref())
        .and_then(|r| r.devices().as_ref())
        // find an item that allows all devices
        .map(|devs| {
            devs.iter()
                .any(|dev| dev.allow() && is_linux_devcg_allowed_all(dev))
        })
        .unwrap_or_default()
}

/// Grant devices cgroup the default permissions.
pub fn allow_default_devices_in_cgroup(manager: &mut dyn Manager) -> Result<()> {
    let mut list = vec![
        // Deny all to reset the device cgroup settings
        new_device_cgroup(false, Some(LinuxDeviceType::A), None, None, Some("rwm"))?,
        // Allow all mknod to all char devices
        new_device_cgroup(true, Some(LinuxDeviceType::C), None, None, Some("m"))?,
        // Allow all mknod to all block devices
        new_device_cgroup(true, Some(LinuxDeviceType::B), None, None, Some("m"))?,
        // Allow all read/write/mknod to char device /dev/console
        new_device_cgroup(
            true,
            Some(LinuxDeviceType::C),
            Some(5), // major for console
            Some(1), // minor for console
            Some("rwm"),
        )?,
        // Allow all read/write/mknod to char device /dev/pts/<N>
        new_device_cgroup(
            true,
            Some(LinuxDeviceType::C),
            Some(136), // major for pts
            None,      // minor is not specified, so all minors are allowed
            Some("rwm"),
        )?,
        // Allow all read/write/mknod to char device /dev/ptmx
        new_device_cgroup(
            true,
            Some(LinuxDeviceType::C),
            Some(5), // major for ptmx
            Some(2), // minor for ptmx
            Some("rwm"),
        )?,
        // Allow all read/write/mknod to char device /dev/net/tun
        new_device_cgroup(
            true,
            Some(LinuxDeviceType::C),
            Some(10),  // major for tun
            Some(200), // minor for tun
            Some("rwm"),
        )?,
    ];

    for dev in DEFAULT_DEVICES.iter() {
        list.push(new_device_cgroup(
            true,
            Some(dev.typ()),
            Some(dev.major()),
            Some(dev.minor()),
            Some("rwm"),
        )?);
    }

    let resources = LinuxResourcesBuilder::default()
        .devices(list)
        .build()
        .context("build LinuxResources")?;

    manager
        .set(&resources)
        .context("set default device cgroup")?;

    Ok(())
}

/// Grant devices cgroup the allowed all permissions.
pub fn allow_all_devices_in_cgroup(manager: &mut dyn Manager) -> Result<()> {
    // Insert two rules: `b *:* rwm` and `c *:* rwm`.
    // The reason of not inserting `a *:* rwm` is that the Linux kernel
    // will deny writing `a` to `devices.allow` once a cgroup has children.
    // You can refer to
    // https://www.kernel.org/doc/Documentation/cgroup-v1/devices.txt.
    let list = vec![
        new_device_cgroup(true, Some(LinuxDeviceType::B), None, None, Some("rwm"))?,
        new_device_cgroup(true, Some(LinuxDeviceType::C), None, None, Some("rwm"))?,
    ];

    let resources = LinuxResourcesBuilder::default()
        .devices(list)
        .build()
        .context("build LinuxResources")?;

    manager
        .set(&resources)
        .context("set universal device cgroup")?;

    Ok(())
}
