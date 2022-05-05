// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file

//! Error codes for the virtual machine monitor subsystem.

/// Errors associated with starting the instance.
#[derive(Debug, thiserror::Error)]
pub enum StartMicrovmError {
    /// The device manager was not configured.
    #[error("the device manager failed to manage devices: {0}")]
    DeviceManager(#[source] crate::device_manager::DeviceMgrError),

    /// Cannot add devices to the Legacy I/O Bus.
    #[error("failure in managing legacy device: {0}")]
    LegacyDevice(#[source] crate::device_manager::LegacyDeviceError),

    /// Cannot read from an Event file descriptor.
    #[error("failure while reading from EventFd file descriptor")]
    EventFd,
}
