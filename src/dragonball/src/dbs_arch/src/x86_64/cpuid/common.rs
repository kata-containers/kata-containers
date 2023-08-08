// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::arch::x86_64::{CpuidResult, __cpuid_count, __get_cpuid_max};

use super::cpu_leaf::*;

pub(crate) const VENDOR_ID_INTEL: &[u8; 12] = b"GenuineIntel";
pub(crate) const VENDOR_ID_AMD: &[u8; 12] = b"AuthenticAMD";
pub(crate) const VENDOR_ID_HYGON: &[u8; 12] = b"HygonGenuine";

#[derive(Clone, Debug)]
pub enum Error {
    InvalidParameters(String),
    NotSupported,
}

/// Get CPUID value for (`function`, `count`).
pub fn get_cpuid(function: u32, count: u32) -> Result<CpuidResult, Error> {
    #[cfg(target_env = "sgx")]
    {
        return Err(Error::NotSupported);
    }

    // TODO: replace with validation based on `has_cpuid()` when it becomes stable:
    //  https://doc.rust-lang.org/core/arch/x86/fn.has_cpuid.html
    // this is safe because the host supports the `cpuid` instruction
    let max_function = unsafe { __get_cpuid_max(function & leaf_0x80000000::LEAF_NUM).0 };
    if function > max_function {
        return Err(Error::InvalidParameters(format!(
            "Function not supported: 0x{function:x}",
        )));
    }

    // this is safe because the host supports the `cpuid` instruction
    let entry = unsafe { __cpuid_count(function, count) };
    if entry.eax == 0 && entry.ebx == 0 && entry.ecx == 0 && entry.edx == 0 {
        return Err(Error::InvalidParameters(format!("Invalid count: {count}")));
    }

    Ok(entry)
}

/// Extracts the CPU vendor id from leaf 0x0.
pub fn get_vendor_id() -> Result<[u8; 12], Error> {
    let vendor_entry = get_cpuid(0, 0)?;
    let bytes: [u8; 12] =
        unsafe { std::mem::transmute([vendor_entry.ebx, vendor_entry.edx, vendor_entry.ecx]) };

    Ok(bytes)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn get_topoext_fn() -> u32 {
        let vendor_id = get_vendor_id();
        assert!(vendor_id.is_ok());
        let function = match &vendor_id.ok().unwrap() {
            VENDOR_ID_INTEL => leaf_0x4::LEAF_NUM,
            VENDOR_ID_AMD => leaf_0x8000001d::LEAF_NUM,
            _ => 0,
        };
        assert!(function != 0);

        function
    }

    #[test]
    fn test_get_cpu_id() {
        // get_cpu_id should work correctly here
        let topoext_fn = get_topoext_fn();

        // check that get_cpuid works for valid parameters
        match get_cpuid(topoext_fn, 0) {
            Ok(topoext_entry) => {
                assert!(topoext_entry.eax != 0);
            }
            _ => panic!("Wrong behavior"),
        }

        // check that get_cpuid returns correct error for invalid `function`
        match get_cpuid(0x9000_0000, 0) {
            Err(Error::InvalidParameters(s)) => {
                assert!(s == "Function not supported: 0x90000000");
            }
            _ => panic!("Wrong behavior"),
        }

        // check that get_cpuid returns correct error for invalid `count`
        match get_cpuid(topoext_fn, 100) {
            Err(Error::InvalidParameters(s)) => {
                assert!(s == "Invalid count: 100");
            }
            _ => panic!("Wrong behavior"),
        }
    }

    #[test]
    fn test_get_vendor_id() {
        let vendor_id = get_vendor_id().unwrap();
        assert!(matches!(&vendor_id, VENDOR_ID_INTEL | VENDOR_ID_AMD));
    }
}
