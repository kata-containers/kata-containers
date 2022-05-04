// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Device manager to manage IO devices for a virtual machine.

/// Virtual machine console device manager.
pub mod console_manager;
/// Console Manager for virtual machines console device.
pub use self::console_manager::ConsoleManager;

/// Errors related to device manager operations.
#[derive(Debug, thiserror::Error)]
pub enum DeviceMgrError {
    /// Failed to manage console devices.
    #[error(transparent)]
    ConsoleManager(console_manager::ConsoleManagerError),
}

/// Specialized version of `std::result::Result` for device manager operations.
pub type Result<T> = ::std::result::Result<T, DeviceMgrError>;
