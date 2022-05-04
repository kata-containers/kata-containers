// Copyright (C) 2018-2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Dragonball is a light-weight virtual machine manager(VMM) based on Linux Kernel-based Virtual
//! Machine(KVM) which is optimized for container workloads.

#![warn(missing_docs)]

/// Address space manager for virtual machines.
pub mod address_space_manager;
/// Device manager for virtual machines.
pub mod device_manager;
/// Resource manager for virtual machines.
pub mod resource_manager;
/// Virtual machine manager for virtual machines.
pub mod vm;
