// Copyright 2021 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! CPU architecture specific constants and utilities for the `x86_64` architecture.

/// Definitions for x86 CPUID
pub mod cpuid;
/// Definitions for x86 Global Descriptor Table
pub mod gdt;
/// Definitions for x86 interrupts
pub mod interrupts;
/// Definitions for x86 Model Specific Registers(MSR).
pub mod msr;
/// Definitions for x86 Registers
pub mod regs;
