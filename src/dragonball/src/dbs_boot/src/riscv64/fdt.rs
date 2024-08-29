// Copyright 2024 Alibaba Cloud. All Rights Reserved.
// Copyright © 2024, Institute of Software, CAS. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Create Flatten Device Tree (FDT) for RISC-V 64-bit systems.

use std::collections::HashMap;
use std::fmt::Debug;

use dbs_arch::aia::AIADevice;
use dbs_arch::{DeviceInfoForFDT, DeviceType};

use vm_fdt::FdtWriter;
use vm_memory::GuestMemoryRegion;
use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};

use super::fdt_utils::*;
use super::Error;
use crate::Result;

const CPU_BASE_PHANDLE: u32 = 0x100;

const AIA_APLIC_PHANDLE: u32 = 2;
const AIA_IMSIC_PHANDLE: u32 = 3;
const CPU_INTC_BASE_PHANDLE: u32 = 4;
// Read the documentation specified when appending the root node to the FDT.
const ADDRESS_CELLS: u32 = 0x2;
const SIZE_CELLS: u32 = 0x2;

/// Creates the flattened device tree for this riscv64 microVM.
pub fn create_fdt<T>(
    fdt_vm_info: FdtVmInfo,
    fdt_device_info: FdtDeviceInfo<T>,
) -> Result<Vec<u8>>
where
    T: DeviceInfoForFDT + Clone + Debug,
{
    // Allocate stuff necessary for storing the blob.
    let mut fdt = FdtWriter::new()?;

    // For an explanation why these nodes were introduced in the blob take a look at
    // https://github.com/devicetree-org/devicetree-specification/releases/tag/v0.4
    // In chapter 3.

    // Header or the root node as per above mentioned documentation.
    let root = fdt.begin_node("")?;
    fdt.property_string("compatible", "linux,dummy-virt")?;
    // For info on #address-cells and size-cells resort to Table 3.1 Root Node
    // Properties
    fdt.property_u32("#address-cells", ADDRESS_CELLS)?;
    fdt.property_u32("#size-cells", SIZE_CELLS)?;
    create_cpu_nodes(&mut fdt, &fdt_vm_info)?;
    create_memory_node(&mut fdt, fdt_vm_info.get_guest_memory())?;
    create_chosen_node(&mut fdt, &fdt_vm_info)?;
    create_aia_node(&mut fdt, fdt_device_info.get_irqchip())?;
    fdt_device_info
        .get_mmio_device_info()
        .map_or(Ok(()), |v| create_devices_node(&mut fdt, v))?;

    // End Header node.
    fdt.end_node(root)?;

    // Allocate another buffer so we can format and then write fdt to guest.
    let fdt_final = fdt.finish()?;

    // Write FDT to memory.
    let fdt_address = GuestAddress(super::get_fdt_addr(fdt_vm_info.get_guest_memory()));
    fdt_vm_info
        .get_guest_memory()
        .write_slice(fdt_final.as_slice(), fdt_address)?;
    Ok(fdt_final)
}

// Following are the auxiliary function for creating the different nodes that we append to our FDT.
fn create_cpu_nodes(fdt: &mut FdtWriter, fdt_vm_info: &FdtVmInfo) -> Result<()> {
    // See https://elixir.bootlin.com/linux/v6.10/source/Documentation/devicetree/bindings/riscv/cpus.yaml
    let cpus = fdt.begin_node("cpus")?;
    // As per documentation, on RISC-V 64-bit systems value should be set to 1.
    fdt.property_u32("#address-cells", 0x01)?;
    fdt.property_u32("#size-cells", 0x0)?;
    // Retrieve CPU frequency from cpu timer regs
    let timebase_frequency: u32 = 369999;
    fdt.property_u32("timebase-frequency", timebase_frequency);

    let num_cpus = fdt_vm_info.get_vcpu_num();
    for cpu_index in 0..num_cpus {
        let cpu = fdt.begin_node(&format!("cpu@{:x}", cpu_index))?;
        fdt.property_string("device_type", "cpu")?;
        fdt.property_string("compatible", "riscv")?;
        fdt.property_string("mmy-type", "sv48")?;
        fdt.property_string("riscv,isa", "rv64iafdcsu_smaia_ssaia")?;
        fdt.property_string("status", "okay")?;
        fdt.property_u64("reg", cpu_index as u64)?;
        fdt.property_u32("phandle", CPU_BASE_PHANDLE + cpu_index)?;
        fdt.end_node(cpu)?;

        // interrupt controller node
        let intc_node = fdt.begin_node("interrupt-controller")?;
        fdt.property_string("compatible", "riscv,cpu-intc")?;
        fdt.property_u32("#interrupt-cells", 1u32)?;
        fdt.property_array_u32("interrupt-controller", &Vec::new())?;
        fdt.property_u32("phandle", CPU_INTC_BASE_PHANDLE + cpu_index)?;
        fdt.end_node(intc_node)?;
    }
    fdt.end_node(cpus)?;

    Ok(())
}

fn create_memory_node<M: GuestMemory>(fdt: &mut FdtWriter, guest_mem: &M) -> Result<()> {
    unimplemented!()
}

fn create_chosen_node(fdt: &mut FdtWriter, fdt_vm_info: &FdtVmInfo) -> Result<()> {
    unimplemented!()
}

fn create_aia_node(fdt: &mut FdtWriter, aia_device: &dyn AIADevice) -> Result<()> {
    unimplemented!()
}

fn create_virtio_node<T: DeviceInfoForFDT + Clone + Debug>(
    fdt: &mut FdtWriter,
    dev_info: &T,
) -> Result<()> {
    unimplemented!()
}

fn create_serial_node<T: DeviceInfoForFDT + Clone + Debug>(
    fdt: &mut FdtWriter,
    dev_info: &T,
) -> Result<()> {
    unimplemented!()
}

fn create_rtc_node<T: DeviceInfoForFDT + Clone + Debug>(
    fdt: &mut FdtWriter,
    dev_info: &T,
) -> Result<()> {
    unimplemented!()
}

fn create_devices_node<T: DeviceInfoForFDT + Clone + Debug>(
    fdt: &mut FdtWriter,
    dev_info: &HashMap<(DeviceType, String), T>,
) -> Result<()> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
}
