// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file

//! Error codes for the virtual machine monitor subsystem.

#[cfg(feature = "dbs-virtio-devices")]
use dbs_virtio_devices::Error as VirtIoError;

use crate::device_manager;

/// Errors associated with starting the instance.
#[derive(Debug, thiserror::Error)]
pub enum StartMicrovmError {
    /// Cannot read from an Event file descriptor.
    #[error("failure while reading from EventFd file descriptor")]
    EventFd,

    /// The device manager was not configured.
    #[error("the device manager failed to manage devices: {0}")]
    DeviceManager(#[source] device_manager::DeviceMgrError),

    /// Cannot add devices to the Legacy I/O Bus.
    #[error("failure in managing legacy device: {0}")]
    LegacyDevice(#[source] device_manager::LegacyDeviceError),

    #[cfg(feature = "virtio-vsock")]
    /// Failed to create the vsock device.
    #[error("cannot create virtio-vsock device: {0}")]
    CreateVsockDevice(#[source] VirtIoError),

    #[cfg(feature = "virtio-vsock")]
    /// Cannot initialize a MMIO Vsock Device or add a device to the MMIO Bus.
    #[error("failure while registering virtio-vsock device: {0}")]
    RegisterVsockDevice(#[source] device_manager::DeviceMgrError),
}
