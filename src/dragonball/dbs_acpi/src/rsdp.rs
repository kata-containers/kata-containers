// Copyright (c) 2019 Intel Corporation
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
// RSDP (Root System Description Pointer) is a data structure used in the ACPI programming interface.
use vm_memory::ByteValued;

#[repr(packed)]
#[derive(Clone, Copy, Default)]
pub struct Rsdp {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    _rsdt_addr: u32,
    pub length: u32,
    pub xsdt_addr: u64,
    pub extended_checksum: u8,
    _reserved: [u8; 3],
}

// SAFETY: Rsdp only contains a series of integers
unsafe impl ByteValued for Rsdp {}

impl Rsdp {
    pub fn new(xsdt_addr: u64) -> Self {
        let mut rsdp = Rsdp {
            signature: *b"RSD PTR ",
            checksum: 0,
            oem_id: *b"ALICLD",
            revision: 1,
            _rsdt_addr: 0,
            length: std::mem::size_of::<Rsdp>() as u32,
            xsdt_addr,
            extended_checksum: 0,
            _reserved: [0; 3],
        };
        rsdp.checksum = super::generate_checksum(&rsdp.as_slice()[0..19]);
        rsdp.extended_checksum = super::generate_checksum(rsdp.as_slice());
        rsdp
    }

    pub fn len() -> usize {
        std::mem::size_of::<Rsdp>()
    }
}
#[cfg(test)]
mod tests {
    use super::Rsdp;
    use vm_memory::bytes::ByteValued;
    #[test]
    fn test_rsdp() {
        let rsdp = Rsdp::new(0xa0000);
        let sum = rsdp
            .as_slice()
            .iter()
            .fold(0u8, |acc, x| acc.wrapping_add(*x));
        assert_eq!(sum, 0);
    }
}
