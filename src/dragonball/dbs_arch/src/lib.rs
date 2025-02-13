// Copyright 2021-2022 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![deny(missing_docs)]

//! CPU architecture specific constants, structures and utilities.
//!
//! This crate provides CPU architecture specific constants, structures and utilities to abstract
//! away CPU architecture specific details from the Dragonball Secure Sandbox or other VMMs.
//!
//! # Supported CPU Architectures
//! - **x86_64**: x86_64 (also known as x64, x86-64, AMD64, and Intel 64) is a 64-bit
//!   version of the x86 instruction set.
//! - **ARM64**: AArch64 or ARM64 is the 64-bit extension of the ARM architecture.

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

/// Enum indicating vpmu feature level
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum VpmuFeatureLevel {
    /// Disabled means vpmu feature is off (by default)
    Disabled,
    /// LimitedlyEnabled means minimal vpmu counters are supported( only cycles and instructions )
    /// For aarch64, LimitedlyEnabled isn't supported currently. The ability will be implemented in the future.
    LimitedlyEnabled,
    /// FullyEnabled means all vpmu counters are supported
    FullyEnabled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_trait() {
        let level = VpmuFeatureLevel::Disabled;
        assert_eq!(format!("{level:#?}"), "Disabled");

        let level = VpmuFeatureLevel::LimitedlyEnabled;
        assert_eq!(format!("{level:#?}"), "LimitedlyEnabled");

        let level = VpmuFeatureLevel::FullyEnabled;
        assert_eq!(format!("{level:#?}"), "FullyEnabled");
    }

    #[test]
    fn test_eq_trait() {
        let level = VpmuFeatureLevel::Disabled;
        assert!(level == VpmuFeatureLevel::Disabled);
        assert!(level != VpmuFeatureLevel::LimitedlyEnabled);
    }

    #[test]
    fn test_copy_trait() {
        let level1 = VpmuFeatureLevel::Disabled;
        let level2 = level1;
        assert_eq!(level1, level2);
    }
}
