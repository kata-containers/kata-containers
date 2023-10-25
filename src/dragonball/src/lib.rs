// Copyright (C) 2018-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Dragonball is a light-weight virtual machine manager(VMM) based on Linux Kernel-based Virtual
//! Machine(KVM) which is optimized for container workloads.

#![warn(missing_docs)]
//TODO: Remove this, after the rest of dragonball has been committed.
#![allow(dead_code)]

#[macro_use]
extern crate lazy_static;

/// Address space manager for virtual machines.
pub mod address_space_manager;
/// API to handle vmm requests.
pub mod api;
/// Structs to maintain configuration information.
pub mod config_manager;
/// Device manager for virtual machines.
pub mod device_manager;
/// Errors related to Virtual machine manager.
pub mod error;
/// Prometheus Metrics.
pub mod hypervisor_metrics;
/// KVM operation context for virtual machines.
pub mod kvm_context;
/// Metrics system.
pub mod metric;
/// Resource manager for virtual machines.
pub mod resource_manager;
/// Signal handler for virtual machines.
pub mod signal_handler;
/// Dragonball Tracer.
pub mod tracer;
/// Virtual CPU manager for virtual machines.
pub mod vcpu;
/// Virtual machine manager for virtual machines.
pub mod vm;

mod event_manager;
mod io_manager;

mod test_utils;

mod vmm;

pub use self::error::StartMicroVmError;
pub use self::io_manager::IoManagerCached;
pub use self::vmm::Vmm;

/// Success exit code.
pub const EXIT_CODE_OK: u8 = 0;
/// Generic error exit code.
pub const EXIT_CODE_GENERIC_ERROR: u8 = 1;
/// Generic exit code for an error considered not possible to occur if the program logic is sound.
pub const EXIT_CODE_UNEXPECTED_ERROR: u8 = 2;
/// Dragonball was shut down after intercepting a restricted system call.
pub const EXIT_CODE_BAD_SYSCALL: u8 = 148;
/// Dragonball was shut down after intercepting `SIGBUS`.
pub const EXIT_CODE_SIGBUS: u8 = 149;
/// Dragonball was shut down after intercepting `SIGSEGV`.
pub const EXIT_CODE_SIGSEGV: u8 = 150;
/// Invalid json passed to the Dragonball process for configuring microvm.
pub const EXIT_CODE_INVALID_JSON: u8 = 151;
/// Bad configuration for microvm's resources, when using a single json.
pub const EXIT_CODE_BAD_CONFIGURATION: u8 = 152;
/// Command line arguments parsing error.
pub const EXIT_CODE_ARG_PARSING: u8 = 153;
