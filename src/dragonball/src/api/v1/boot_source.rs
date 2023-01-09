// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use serde_derive::{Deserialize, Serialize};

/// Default guest kernel command line:
/// - `reboot=k` shutdown the guest on reboot, instead of well... rebooting;
/// - `panic=1` on panic, reboot after 1 second;
/// - `pci=off` do not scan for PCI devices (ser boot time);
/// - `nomodules` disable loadable kernel module support;
/// - `8250.nr_uarts=0` disable 8250 serial interface;
/// - `i8042.noaux` do not probe the i8042 controller for an attached mouse (ser boot time);
/// - `i8042.nomux` do not probe i8042 for a multiplexing controller (ser boot time);
/// - `i8042.nopnp` do not use ACPIPnP to discover KBD/AUX controllers (ser boot time);
/// - `i8042.dumbkbd` do not attempt to control kbd state via the i8042 (ser boot time).
pub const DEFAULT_KERNEL_CMDLINE: &str = "reboot=k panic=1 pci=off nomodules 8250.nr_uarts=0 \
                                          i8042.noaux i8042.nomux i8042.nopnp i8042.dumbkbd";

/// Strongly typed data structure used to configure the boot source of the microvm.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct BootSourceConfig {
    /// Path of the kernel image.
    /// We only support uncompressed kernel for Dragonball.
    pub kernel_path: String,
    /// Path of the initrd, if there is one.
    /// ps. rootfs is set in BlockDeviceConfigInfo
    pub initrd_path: Option<String>,
    /// The boot arguments to pass to the kernel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_args: Option<String>,
}

/// Errors associated with actions on `BootSourceConfig`.
#[derive(Debug, thiserror::Error)]
pub enum BootSourceConfigError {
    /// The kernel file cannot be opened.
    #[error(
        "the kernel file cannot be opened due to invalid kernel path or invalid permissions: {0}"
    )]
    InvalidKernelPath(#[source] std::io::Error),

    /// The initrd file cannot be opened.
    #[error("the initrd file cannot be opened due to invalid path or invalid permissions: {0}")]
    InvalidInitrdPath(#[source] std::io::Error),

    /// The kernel command line is invalid.
    #[error("the kernel command line is invalid: {0}")]
    InvalidKernelCommandLine(#[source] linux_loader::cmdline::Error),

    /// The boot source cannot be update post boot.
    #[error("the update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,
}
