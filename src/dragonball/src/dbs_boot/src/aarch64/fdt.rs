// Copyright 2022 Alibaba Cloud. All Rights Reserved.
// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Create Flatten Device Tree (FDT) for ARM64 systems.

use std::collections::HashMap;
use std::fmt::Debug;

use dbs_arch::gic::its::ItsType::{self, PciMsiIts, PlatformMsiIts};
use dbs_arch::gic::GICDevice;
use dbs_arch::{pmu::VIRTUAL_PMU_IRQ, VpmuFeatureLevel};
use dbs_arch::{DeviceInfoForFDT, DeviceType};

use vm_fdt::{FdtWriter, FdtWriterNode};
use vm_memory::GuestMemoryRegion;
use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};

use super::cache_info::{cache_entry::CacheEntry, read_cache_config};
use super::fdt_utils::*;
use super::Error;
use crate::Result;

// This is a value for uniquely identifying the FDT node declaring the interrupt controller.
const GIC_PHANDLE: u32 = 1;
// This is a value for uniquely identifying the FDT node containing the clock definition.
const CLOCK_PHANDLE: u32 = 2;
// This is a value for uniquely identifying the FDT node containing the plaform msi ITS definition.
const GIC_PLATFORM_MSI_ITS_PHANDLE: u32 = 3;
// This is a value for uniquely identifying the FDT node containing the pci msi ITS definition.
const GIC_PCI_MSI_ITS_PHANDLE: u32 = 4;
// According to the arm, gic-v3.txt document, ITS' #msi-cells is fixed at 1.
const GIC_PLATFORM_MSI_ITS_CELLS_SIZE: u32 = 1;
// You may be wondering why this big value?
// This phandle is used to uniquely identify the FDT nodes containing cache information. Each cpu
// can have a variable number of caches, some of these caches may be shared with other cpus.
// So, we start the indexing of the phandles used from a really big number and then substract from
// it as we need more and more phandle for each cache representation.
const LAST_CACHE_PHANDLE: u32 = 4000;

// Read the documentation specified when appending the root node to the FDT.
const ADDRESS_CELLS: u32 = 0x2;
const SIZE_CELLS: u32 = 0x2;

// As per kvm tool and
// https://www.kernel.org/doc/Documentation/devicetree/bindings/interrupt-controller/arm%2Cgic.txt
// Look for "The 1st cell..."
const GIC_FDT_IRQ_TYPE_SPI: u32 = 0;
const GIC_FDT_IRQ_TYPE_PPI: u32 = 1;

// From https://elixir.bootlin.com/linux/v4.9.62/source/include/dt-bindings/interrupt-controller/irq.h#L17
const IRQ_TYPE_EDGE_RISING: u32 = 1;
const IRQ_TYPE_LEVEL_HI: u32 = 4;

/// Creates the flattened device tree for this aarch64 microVM.
pub fn create_fdt<T>(
    fdt_vm_info: FdtVmInfo,
    fdt_numa_info: FdtNumaInfo,
    fdt_device_info: FdtDeviceInfo<T>,
) -> Result<Vec<u8>>
where
    T: DeviceInfoForFDT + Clone + Debug,
{
    let mut fdt = FdtWriter::new()?;

    // For an explanation why these nodes were introduced in the blob take a look at
    // https://github.com/torvalds/linux/blob/master/Documentation/devicetree/booting-without-of.txt#L845
    // Look for "Required nodes and properties".

    // Header or the root node as per above mentioned documentation.
    let root_node = fdt.begin_node("")?;
    fdt.property_string("compatible", "linux,dummy-virt")?;
    // For info on #address-cells and size-cells read "Note about cells and address representation"
    // from the above mentioned txt file.
    fdt.property_u32("#address-cells", ADDRESS_CELLS)?;
    fdt.property_u32("#size-cells", SIZE_CELLS)?;
    // This is not mandatory but we use it to point the root node to the node
    // containing description of the interrupt controller for this VM.
    fdt.property_u32("interrupt-parent", GIC_PHANDLE)?;
    create_cpu_nodes(&mut fdt, &fdt_vm_info, &fdt_numa_info)?;
    create_memory_node(
        &mut fdt,
        fdt_vm_info.get_guest_memory(),
        fdt_numa_info.get_memory_numa_id_map(),
    )?;
    create_chosen_node(&mut fdt, &fdt_vm_info)?;
    create_gic_node(&mut fdt, fdt_device_info.get_irqchip())?;
    create_timer_node(&mut fdt)?;
    create_clock_node(&mut fdt)?;
    create_psci_node(&mut fdt)?;
    fdt_device_info
        .get_mmio_device_info()
        .map_or(Ok(()), |v| create_devices_node(&mut fdt, v))?;
    create_pmu_node(&mut fdt, fdt_vm_info.get_vpmu_feature())?;

    // End Header node.
    fdt.end_node(root_node)?;

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
fn create_cpu_nodes(
    fdt: &mut FdtWriter,
    fdt_vm_info: &FdtVmInfo,
    fdt_numa_info: &FdtNumaInfo,
) -> Result<()> {
    // See https://github.com/torvalds/linux/blob/master/Documentation/devicetree/bindings/arm/cpus.yaml.
    let cpus_node = fdt.begin_node("cpus")?;
    // As per documentation, on ARM v8 64-bit systems value should be set to 2.
    fdt.property_u32("#address-cells", 0x02)?;
    fdt.property_u32("#size-cells", 0x0)?;
    let vcpu_mpidr = fdt_vm_info.get_vcpu_mpidr();
    let vcpu_boot_onlined = fdt_vm_info.get_boot_onlined();
    let num_cpus = vcpu_mpidr.len();
    let cache_info = if fdt_vm_info.get_cache_passthrough_enabled() {
        // Unwrap cpu_maps here is safe because it is Some(value) only when cache_passthrough_enabled is true.
        // The vmm should ensure the this condition.
        read_cache_config(fdt_numa_info.get_cpu_maps().unwrap(), Some(3))
            .map_err(Error::ReadCacheInfoError)?
    } else {
        HashMap::new()
    };
    let vcpu_l3_cache_map = fdt_numa_info.get_vcpu_l3_cache_map();
    let mut non_l1_cache_node_count = 0u32;

    for (cpu_index, mpidr) in vcpu_mpidr.iter().enumerate().take(num_cpus) {
        let cpu_name = format!("cpu@{cpu_index:x}");
        let cpu_node = fdt.begin_node(&cpu_name)?;
        fdt.property_string("device_type", "cpu")?;
        fdt.property_string("compatible", "arm,arm-v8")?;
        if num_cpus > 1 {
            // This is required on armv8 64-bit. See aforementioned documentation.
            fdt.property_string("enable-method", "psci")?;
        }
        // boot-onlined attribute is used to indicate whether this cpu should be onlined at boot.
        // 0 means offline, 1 means online.
        fdt.property_u32("boot-onlined", vcpu_boot_onlined[cpu_index])?;
        // Set the field to first 24 bits of the MPIDR - Multiprocessor Affinity Register.
        // See http://infocenter.arm.com/help/index.jsp?topic=/com.arm.doc.ddi0488c/BABHBJCI.html.
        fdt.property_u64("reg", mpidr & 0x7FFFFF)?;

        // See https://github.com/torvalds/linux/blob/master/Documentation/devicetree/bindings/numa.txt
        let cpu_numa_id_map = fdt_numa_info.get_vcpu_numa_id_map();
        // Check index here because in cpu hotplug scenario, the length of vcpu_mpidr equals max_vcpu_count,
        // while numa information is set at boot. To be compatible with this circumstance, add a check here
        // to ensure the index won't overflow.
        if cpu_index < cpu_numa_id_map.len() {
            fdt.property_u32("numa-node-id", cpu_numa_id_map[cpu_index])?;
        }

        // Currently, dragonball only support binding vcpu on numa level, which means the cache information
        // can be assured correct on numa level.
        if !cache_info.is_empty() && cache_info.contains_key(&cpu_index) {
            // Append L1 cache information to each cpu node.
            append_l1_cache_property(fdt, cache_info.get(&cpu_index).unwrap().0.as_slice())?;
            // Append L2/L3 cache node into each cpu node.
            let is_append_l3_cache = vcpu_l3_cache_map[cpu_index] == (cpu_index as u32);
            create_non_l1_cache_node(
                fdt,
                cache_info.get(&cpu_index).unwrap().1.as_slice(),
                num_cpus,
                cpu_index,
                is_append_l3_cache,
                non_l1_cache_node_count,
            )?;
            if is_append_l3_cache {
                non_l1_cache_node_count += 1;
            }
        }

        fdt.end_node(cpu_node)?;
    }
    fdt.end_node(cpus_node)?;

    Ok(())
}

fn append_l1_cache_property(fdt: &mut FdtWriter, l1_caches: &[CacheEntry]) -> Result<()> {
    // Please check out
    // https://github.com/devicetree-org/devicetree-specification/releases/download/v0.3/devicetree-specification-v0.3.pdf,
    // section 3.8.
    // L1 cache contians l1-instruction-cache and l1-data-cache
    for cache in l1_caches.iter() {
        if let Some(size) = cache.size_ {
            fdt.property_u32(cache.type_.of_cache_size(), size as u32)?;
        }
        if let Some(line_size) = cache.line_size {
            fdt.property_u32(cache.type_.of_cache_line_size(), line_size as u32)?;
        }
        if let Some(number_of_sets) = cache.number_of_sets {
            fdt.property_u32(cache.type_.of_cache_sets(), number_of_sets)?;
        }
    }

    Ok(())
}

fn create_non_l1_cache_node(
    fdt: &mut FdtWriter,
    non_l1_caches: &[CacheEntry],
    num_cpus: usize,
    cpu_index: usize,
    is_append_l3_cache: bool,
    non_l1_cache_count: u32,
) -> Result<()> {
    // Some of the non-l1 caches can be shared amongst CPUs. You can see an example of a shared
    // scenario in https://github.com/devicetree-org/devicetree-specification/releases/download/v0.3/devicetree-specification-v0.3.pdf,
    // 3.8.1 Example.
    let mut prev_level = 1;
    // Initialize a l2_cache_node for fdt.end_node.
    // As non-l1 caches contains two entries: l2 cache and l3 cache. So the variable will
    // only be assigned once in the loop. To be compatible with vm_fdt crate, create a
    // temporary FdtWriter for passing compiling check.
    let mut l2_cache_node = FdtWriter::new()?.begin_node("TEMP")?;

    for cache in non_l1_caches.iter() {
        // We append the next-level-cache property (the node that specifies the cache hierarchy)
        // in the next iteration. For example,
        // L2-cache {
        //      cache-size = <0x8000> ----> first iteration
        //      next-level-cache = <&l3-cache> ---> second iteration
        // }
        // The cpus per unit cannot be 0 since the sysfs will also include the current cpu
        // in the list of shared cpus so it needs to be at least 1. Firecracker trusts the host.
        // The operation is safe since we already checked when creating cache attributes that
        // cpus_per_unit is not 0 (.e look for mask_str2bit_count function).
        let cache_phandle = if cache.level == 2 {
            LAST_CACHE_PHANDLE - cpu_index as u32
        } else {
            LAST_CACHE_PHANDLE - (num_cpus as u32 + non_l1_cache_count)
        };

        // Add next-level-cache in the parent node.
        if prev_level != cache.level {
            fdt.property_u32("next-level-cache", cache_phandle)?;
        }

        // Currently, the kernel only expose L0~L3 caches.
        match cache.level {
            // L2 cache.
            2 => {
                l2_cache_node = append_non_l1_cache_node(fdt, cpu_index, cache, cache_phandle)?;
                prev_level = 2;
            }
            // L3 cache.
            3 => {
                // L3 cache only attach to the first cpu node in its
                // shared_cpu_map structure.
                if !is_append_l3_cache {
                    break;
                }
                let l3_cache_node = append_non_l1_cache_node(fdt, cpu_index, cache, cache_phandle)?;
                fdt.end_node(l3_cache_node)?;
                prev_level = 3;
            }
            _ => {}
        }
    }
    fdt.end_node(l2_cache_node)?;

    Ok(())
}

fn append_non_l1_cache_node(
    fdt: &mut FdtWriter,
    cpu_index: usize,
    cache: &CacheEntry,
    cache_phandle: u32,
) -> Result<FdtWriterNode> {
    let cache_node_name = format!(
        "l{}-{}-cache",
        cache.level,
        cpu_index / cache.cpus_per_unit as usize
    );
    let cache_node = fdt.begin_node(cache_node_name.as_str())?;
    fdt.property_phandle(cache_phandle)?;
    fdt.property_string("compatible", "cache")?;
    fdt.property_u32("cache_level", cache.level as u32)?;
    if let Some(cache_type) = cache.type_.of_cache_type() {
        fdt.property_null(cache_type)?;
    }
    if let Some(size) = cache.size_ {
        fdt.property_u32(cache.type_.of_cache_size(), size as u32)?;
    }
    if let Some(line_size) = cache.line_size {
        fdt.property_u32(cache.type_.of_cache_line_size(), line_size as u32)?;
    }
    if let Some(number_of_sets) = cache.number_of_sets {
        fdt.property_u32(cache.type_.of_cache_sets(), number_of_sets)?;
    }

    Ok(cache_node)
}

fn create_memory_node<M: GuestMemory>(
    fdt: &mut FdtWriter,
    guest_mem: &M,
    memory_numa_id: &[u32],
) -> Result<()> {
    // See https://github.com/torvalds/linux/blob/v5.9/Documentation/devicetree/booting-without-of.rst
    for (index, region) in guest_mem.iter().enumerate() {
        let memory_name = format!("memory@{:x}", region.start_addr().raw_value());
        let mem_reg_prop = &[region.start_addr().raw_value(), region.len()];
        let memory_node = fdt.begin_node(&memory_name)?;
        fdt.property_string("device_type", "memory")?;
        fdt.property_array_u64("reg", mem_reg_prop)?;
        // See https://github.com/torvalds/linux/blob/master/Documentation/devicetree/bindings/numa.txt
        let memory_numa_id_map = memory_numa_id;
        if index < memory_numa_id_map.len() {
            fdt.property_u32("numa-node-id", memory_numa_id_map[index])?;
        }
        fdt.end_node(memory_node)?;
    }
    Ok(())
}

fn create_chosen_node(fdt: &mut FdtWriter, fdt_vm_info: &FdtVmInfo) -> Result<()> {
    let chosen_node = fdt.begin_node("chosen")?;
    fdt.property_string("bootargs", fdt_vm_info.get_cmdline())?;

    if let Some(initrd_config) = fdt_vm_info.get_initrd_config() {
        fdt.property_u64("linux,initrd-start", initrd_config.address.raw_value())?;
        fdt.property_u64(
            "linux,initrd-end",
            initrd_config.address.raw_value() + initrd_config.size as u64,
        )?;
    }

    fdt.end_node(chosen_node)?;

    Ok(())
}

fn append_its_common_property(fdt: &mut FdtWriter, registers_prop: &[u64]) -> Result<()> {
    fdt.property_string("compatible", "arm,gic-v3-its")?;
    fdt.property_null("msi-controller")?;
    fdt.property_array_u64("reg", registers_prop)?;
    Ok(())
}

fn create_its_node(
    fdt: &mut FdtWriter,
    gic_device: &dyn GICDevice,
    its_type: ItsType,
) -> Result<()> {
    let reg = gic_device.get_its_reg_range(&its_type);
    if let Some(registers) = reg {
        // There are two types of its, pci_msi_its and platform_msi_its.
        // If this is pci_msi_its, the fdt node of its is required to have no
        // #msi-cells attribute. If this is platform_msi_its, the #msi-cells
        // attribute of its fdt node is required, and the value is 1.
        match its_type {
            PlatformMsiIts => {
                let its_node = fdt.begin_node("gic-platform-its")?;
                append_its_common_property(fdt, &registers)?;
                fdt.property_u32("phandle", GIC_PLATFORM_MSI_ITS_PHANDLE)?;
                fdt.property_u32("#msi-cells", GIC_PLATFORM_MSI_ITS_CELLS_SIZE)?;
                fdt.end_node(its_node)?;
            }
            PciMsiIts => {
                let its_node = fdt.begin_node("gic-pci-its")?;
                append_its_common_property(fdt, &registers)?;
                fdt.property_u32("phandle", GIC_PCI_MSI_ITS_PHANDLE)?;
                fdt.end_node(its_node)?;
            }
        }
    }
    Ok(())
}

fn create_gic_node(fdt: &mut FdtWriter, gic_device: &dyn GICDevice) -> Result<()> {
    let gic_reg_prop = gic_device.device_properties();

    let intc_node = fdt.begin_node("intc")?;
    fdt.property_string("compatible", gic_device.fdt_compatibility())?;
    fdt.property_null("interrupt-controller")?;
    // "interrupt-cells" field specifies the number of cells needed to encode an
    // interrupt source. The type shall be a <u32> and the value shall be 3 if no PPI affinity description
    // is required.
    fdt.property_u32("#interrupt-cells", 3)?;
    fdt.property_array_u64("reg", gic_reg_prop)?;
    fdt.property_u32("phandle", GIC_PHANDLE)?;
    fdt.property_u32("#address-cells", 2)?;
    fdt.property_u32("#size-cells", 2)?;
    fdt.property_null("ranges")?;
    let gic_intr_prop = &[
        GIC_FDT_IRQ_TYPE_PPI,
        gic_device.fdt_maint_irq(),
        IRQ_TYPE_LEVEL_HI,
    ];

    fdt.property_array_u32("interrupts", gic_intr_prop)?;
    create_its_node(fdt, gic_device, PlatformMsiIts)?;
    create_its_node(fdt, gic_device, PciMsiIts)?;
    fdt.end_node(intc_node)?;

    Ok(())
}

fn create_clock_node(fdt: &mut FdtWriter) -> Result<()> {
    // The Advanced Peripheral Bus (APB) is part of the Advanced Microcontroller Bus Architecture
    // (AMBA) protocol family. It defines a low-cost interface that is optimized for minimal power
    // consumption and reduced interface complexity.
    // PCLK is the clock source and this node defines exactly the clock for the APB.
    let clock_node = fdt.begin_node("apb-pclk")?;
    fdt.property_string("compatible", "fixed-clock")?;
    fdt.property_u32("#clock-cells", 0x0)?;
    fdt.property_u32("clock-frequency", 24000000)?;
    fdt.property_string("clock-output-names", "clk24mhz")?;
    fdt.property_u32("phandle", CLOCK_PHANDLE)?;
    fdt.end_node(clock_node)?;

    Ok(())
}

fn create_timer_node(fdt: &mut FdtWriter) -> Result<()> {
    // See
    // https://github.com/torvalds/linux/blob/master/Documentation/devicetree/bindings/interrupt-controller/arch_timer.txt
    // These are fixed interrupt numbers for the timer device.
    let irqs = [13, 14, 11, 10];
    let compatible = "arm,armv8-timer";

    let mut timer_reg_cells: Vec<u32> = Vec::new();
    for &irq in irqs.iter() {
        timer_reg_cells.push(GIC_FDT_IRQ_TYPE_PPI);
        timer_reg_cells.push(irq);
        timer_reg_cells.push(IRQ_TYPE_LEVEL_HI);
    }

    let timer_node = fdt.begin_node("timer")?;
    fdt.property_string("compatible", compatible)?;
    fdt.property_null("always-on")?;
    fdt.property_array_u32("interrupts", &timer_reg_cells)?;
    fdt.end_node(timer_node)?;

    Ok(())
}

fn create_psci_node(fdt: &mut FdtWriter) -> Result<()> {
    let compatible = "arm,psci-0.2";
    let psci_node = fdt.begin_node("psci")?;
    fdt.property_string("compatible", compatible)?;
    // Two methods available: hvc and smc.
    // As per documentation, PSCI calls between a guest and hypervisor may use the HVC conduit instead of SMC.
    // So, since we are using kvm, we need to use hvc.
    fdt.property_string("method", "hvc")?;
    fdt.end_node(psci_node)?;

    Ok(())
}

fn create_virtio_node<T: DeviceInfoForFDT + Clone + Debug>(
    fdt: &mut FdtWriter,
    dev_info: &T,
) -> Result<()> {
    let device_reg_prop = &[dev_info.addr(), dev_info.length()];
    let irq_number = dev_info.irq().map_err(|_| Error::InvalidArguments)?;
    let irq_property = &[GIC_FDT_IRQ_TYPE_SPI, irq_number, IRQ_TYPE_EDGE_RISING];

    let virtio_mmio_node = fdt.begin_node(&format!("virtio_mmio@{:x}", dev_info.addr()))?;
    fdt.property_string("compatible", "virtio,mmio")?;
    fdt.property_array_u64("reg", device_reg_prop)?;
    fdt.property_array_u32("interrupts", irq_property)?;
    fdt.property_u32("interrupt-parent", GIC_PHANDLE)?;
    fdt.end_node(virtio_mmio_node)?;

    Ok(())
}

fn create_serial_node<T: DeviceInfoForFDT + Clone + Debug>(
    fdt: &mut FdtWriter,
    dev_info: &T,
) -> Result<()> {
    let serial_reg_prop = &[dev_info.addr(), dev_info.length()];
    let irq_number = dev_info.irq().map_err(|_| Error::InvalidArguments)?;
    let irq_property = &[GIC_FDT_IRQ_TYPE_SPI, irq_number, IRQ_TYPE_EDGE_RISING];

    let uart_node = fdt.begin_node(&format!("uart@{:x}", dev_info.addr()))?;
    fdt.property_string("compatible", "ns16550a")?;
    fdt.property_array_u64("reg", serial_reg_prop)?;
    fdt.property_u32("clocks", CLOCK_PHANDLE)?;
    fdt.property_string("clock-names", "apb_pclk")?;
    fdt.property_array_u32("interrupts", irq_property)?;
    fdt.end_node(uart_node)?;

    Ok(())
}

fn create_rtc_node<T: DeviceInfoForFDT + Clone + Debug>(
    fdt: &mut FdtWriter,
    dev_info: &T,
) -> Result<()> {
    let compatible = b"arm,pl031\0arm,primecell\0";
    let rtc_reg_prop = &[dev_info.addr(), dev_info.length()];
    let irq_number = dev_info.irq().map_err(|_| Error::InvalidArguments)?;
    let irq_property = &[GIC_FDT_IRQ_TYPE_SPI, irq_number, IRQ_TYPE_LEVEL_HI];

    let rtc_node = fdt.begin_node(&format!("rtc@{:x}", dev_info.addr()))?;
    fdt.property("compatible", compatible)?;
    fdt.property_array_u64("reg", rtc_reg_prop)?;
    fdt.property_array_u32("interrupts", irq_property)?;
    fdt.property_u32("clocks", CLOCK_PHANDLE)?;
    fdt.property_string("clock-names", "apb_pclk")?;
    fdt.end_node(rtc_node)?;

    Ok(())
}

fn create_devices_node<T: DeviceInfoForFDT + Clone + Debug>(
    fdt: &mut FdtWriter,
    dev_info: &HashMap<(DeviceType, String), T>,
) -> Result<()> {
    // Serial devices need to be registered in order
    let mut ordered_serial_device: Vec<&T> = Vec::new();
    // Create one temp Vec to store all virtio devices
    let mut ordered_virtio_device: Vec<&T> = Vec::new();

    for ((device_type, _device_id), info) in dev_info {
        match device_type {
            DeviceType::RTC => create_rtc_node(fdt, info)?,
            DeviceType::Serial => {
                ordered_serial_device.push(info);
            }
            DeviceType::Virtio(_) => {
                ordered_virtio_device.push(info);
            }
        }
    }

    // Sort out serial devices by address from low to high and insert them into fdt table.
    ordered_serial_device.sort_by_key(|a| a.addr());
    for serial_device_info in ordered_serial_device.drain(..) {
        create_serial_node(fdt, serial_device_info)?;
    }
    // Sort out virtio devices by address from low to high and insert them into fdt table.
    ordered_virtio_device.sort_by_key(|a| a.addr());
    for ordered_device_info in ordered_virtio_device.drain(..) {
        create_virtio_node(fdt, ordered_device_info)?;
    }

    Ok(())
}

fn create_pmu_node(fdt: &mut FdtWriter, vpmu_feature: VpmuFeatureLevel) -> Result<()> {
    if vpmu_feature == VpmuFeatureLevel::Disabled {
        return Ok(());
    };

    let pmu_node = fdt.begin_node("pmu")?;
    fdt.property_string("compatible", "arm,armv8-pmuv3")?;
    let pmu_intr_prop = [GIC_FDT_IRQ_TYPE_PPI, VIRTUAL_PMU_IRQ, IRQ_TYPE_LEVEL_HI];
    fdt.property_array_u32("interrupts", &pmu_intr_prop)?;
    fdt.end_node(pmu_node)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cmp::min;
    use std::collections::HashMap;
    use std::env;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::path::PathBuf;

    use dbs_arch::{gic::create_gic, pmu::initialize_pmu};
    use device_tree::DeviceTree;
    use kvm_bindings::{kvm_vcpu_init, KVM_ARM_VCPU_PMU_V3, KVM_ARM_VCPU_PSCI_0_2};
    use kvm_ioctls::{Kvm, VcpuFd, VmFd};
    use vm_memory::GuestMemoryMmap;

    use super::super::tests::MMIODeviceInfo;
    use super::*;
    use crate::layout::{DRAM_MEM_MAX_SIZE, DRAM_MEM_START, FDT_MAX_SIZE};
    use crate::InitrdConfig;

    const LEN: u64 = 4096;

    fn arch_memory_regions(size: usize) -> Vec<(GuestAddress, usize)> {
        let dram_size = min(size as u64, DRAM_MEM_MAX_SIZE) as usize;
        vec![(GuestAddress(DRAM_MEM_START), dram_size)]
    }

    // The `load` function from the `device_tree` will mistakenly check the actual size
    // of the buffer with the allocated size. This works around that.
    fn set_size(buf: &mut [u8], pos: usize, val: usize) {
        buf[pos] = ((val >> 24) & 0xff) as u8;
        buf[pos + 1] = ((val >> 16) & 0xff) as u8;
        buf[pos + 2] = ((val >> 8) & 0xff) as u8;
        buf[pos + 3] = (val & 0xff) as u8;
    }

    // Initialize vcpu for pmu test
    fn initialize_vcpu_with_pmu(vm: &VmFd, vcpu: &VcpuFd) -> Result<()> {
        let mut kvi: kvm_vcpu_init = kvm_vcpu_init::default();
        vm.get_preferred_target(&mut kvi)
            .expect("Cannot get preferred target");
        kvi.features[0] = 1 << KVM_ARM_VCPU_PSCI_0_2 | 1 << KVM_ARM_VCPU_PMU_V3;
        vcpu.vcpu_init(&kvi).map_err(|_| Error::InvalidArguments)?;
        initialize_pmu(vm, vcpu).map_err(|_| Error::InvalidArguments)?;

        Ok(())
    }

    // Create fdt dtb file
    fn create_dtb_file(name: &str, dtb: &[u8]) {
        // Control whether to create new dtb files for unit test.
        // Usage: FDT_CREATE_DTB=1 cargo test
        if env::var("FDT_CREATE_DTB").is_err() {
            return;
        }

        // Use this code when wanting to generate a new DTB sample.
        // Do manually check dtb files with dtc
        // See https://git.kernel.org/pub/scm/utils/dtc/dtc.git/plain/Documentation/manual.txt
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut output = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path.join(format!("src/aarch64/test/{name}")))
            .unwrap();
        output
            .set_len(FDT_MAX_SIZE as u64)
            .map_err(|_| Error::InvalidArguments)
            .unwrap();
        output.write_all(dtb).unwrap();
    }

    #[test]
    fn test_create_fdt_with_devices() {
        let regions = arch_memory_regions(FDT_MAX_SIZE + 0x1000);
        let mem = GuestMemoryMmap::<()>::from_ranges(&regions).expect("Cannot initialize memory");
        let dev_info: HashMap<(DeviceType, String), MMIODeviceInfo> = [
            (
                (DeviceType::Serial, DeviceType::Serial.to_string()),
                MMIODeviceInfo::new(0, 1),
            ),
            (
                (DeviceType::Virtio(1), "virtio".to_string()),
                MMIODeviceInfo::new(LEN, 2),
            ),
            (
                (DeviceType::RTC, "rtc".to_string()),
                MMIODeviceInfo::new(2 * LEN, 3),
            ),
        ]
        .iter()
        .cloned()
        .collect();
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let _ = vm.create_vcpu(0).unwrap();
        let gic = create_gic(&vm, 1).unwrap();
        let vpmu_feature = VpmuFeatureLevel::Disabled;
        assert!(create_fdt(
            FdtVmInfo::new(
                &mem,
                "console=tty0",
                None,
                FdtVcpuInfo::new(vec![0], vec![1], vpmu_feature, false)
            ),
            FdtNumaInfo::default(),
            FdtDeviceInfo::new(Some(&dev_info), gic.as_ref())
        )
        .is_ok())
    }

    #[test]
    fn test_create_fdt() {
        let regions = arch_memory_regions(FDT_MAX_SIZE + 0x1000);
        let mem = GuestMemoryMmap::<()>::from_ranges(&regions).expect("Cannot initialize memory");
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let _ = vm.create_vcpu(0).unwrap();
        let gic = create_gic(&vm, 1).unwrap();
        let vpmu_feature = VpmuFeatureLevel::Disabled;
        let dtb = create_fdt(
            FdtVmInfo::new(
                &mem,
                "console=tty0",
                None,
                FdtVcpuInfo::new(vec![0], vec![1], vpmu_feature, false),
            ),
            FdtNumaInfo::default(),
            FdtDeviceInfo::<MMIODeviceInfo>::new(None, gic.as_ref()),
        )
        .unwrap();

        create_dtb_file("output.dtb", &dtb);

        let bytes = include_bytes!("test/output.dtb");
        let pos = 4;
        let val = FDT_MAX_SIZE;
        let mut buf = vec![];
        buf.extend_from_slice(bytes);
        set_size(&mut buf, pos, val);

        let original_fdt = DeviceTree::load(&buf).unwrap();
        let generated_fdt = DeviceTree::load(&dtb).unwrap();
        assert_eq!(format!("{original_fdt:?}"), format!("{generated_fdt:?}"));
    }

    #[test]
    fn test_create_fdt_with_initrd() {
        let regions = arch_memory_regions(FDT_MAX_SIZE + 0x1000);
        let mem = GuestMemoryMmap::<()>::from_ranges(&regions).expect("Cannot initialize memory");
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let _ = vm.create_vcpu(0).unwrap();
        let gic = create_gic(&vm, 1).unwrap();
        let initrd = InitrdConfig {
            address: GuestAddress(0x10000000),
            size: 0x1000,
        };
        let vpmu_feature = VpmuFeatureLevel::Disabled;
        let dtb = create_fdt(
            FdtVmInfo::new(
                &mem,
                "console=tty0",
                Some(&initrd),
                FdtVcpuInfo::new(vec![0], vec![1], vpmu_feature, false),
            ),
            FdtNumaInfo::default(),
            FdtDeviceInfo::<MMIODeviceInfo>::new(None, gic.as_ref()),
        )
        .unwrap();

        create_dtb_file("output_with_initrd.dtb", &dtb);

        let bytes = include_bytes!("test/output_with_initrd.dtb");
        let pos = 4;
        let val = FDT_MAX_SIZE;
        let mut buf = vec![];
        buf.extend_from_slice(bytes);
        set_size(&mut buf, pos, val);

        let original_fdt = DeviceTree::load(&buf).unwrap();
        let generated_fdt = DeviceTree::load(&dtb).unwrap();
        assert_eq!(format!("{original_fdt:?}"), format!("{generated_fdt:?}"));
    }

    #[test]
    fn test_create_fdt_with_pmu() {
        let regions = arch_memory_regions(FDT_MAX_SIZE + 0x1000);
        let mem = GuestMemoryMmap::<()>::from_ranges(&regions).expect("Cannot initialize memory");
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let gic = create_gic(&vm, 1).unwrap();

        assert!(initialize_vcpu_with_pmu(&vm, &vcpu).is_ok());

        let vpmu_feature = VpmuFeatureLevel::FullyEnabled;
        let dtb = create_fdt(
            FdtVmInfo::new(
                &mem,
                "console=tty0",
                None,
                FdtVcpuInfo::new(vec![0], vec![1], vpmu_feature, false),
            ),
            FdtNumaInfo::default(),
            FdtDeviceInfo::<MMIODeviceInfo>::new(None, gic.as_ref()),
        )
        .unwrap();

        create_dtb_file("output_with_pmu.dtb", &dtb);

        let bytes = include_bytes!("test/output_with_pmu.dtb");
        let pos = 4;
        let val = FDT_MAX_SIZE;
        let mut buf = vec![];
        buf.extend_from_slice(bytes);
        set_size(&mut buf, pos, val);

        let original_fdt = DeviceTree::load(&buf).unwrap();
        let generated_fdt = DeviceTree::load(&dtb).unwrap();
        assert_eq!(format!("{original_fdt:?}"), format!("{generated_fdt:?}"));
    }

    #[test]
    fn test_create_fdt_with_cache() {
        let regions = arch_memory_regions(FDT_MAX_SIZE + 0x1000);
        let mem = GuestMemoryMmap::<()>::from_ranges(&regions).expect("Cannot initialize memory");
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu_id = (0..128).collect::<Vec<u64>>();
        assert!(vcpu_id
            .as_slice()
            .iter()
            .enumerate()
            .map(|(id, _)| vm.create_vcpu(id as u64))
            .all(|result| result.is_ok()));
        let gic = create_gic(&vm, 128).unwrap();
        let vpmu_feature = VpmuFeatureLevel::Disabled;
        let cpu_maps = Some((0..128).collect::<Vec<u8>>());
        let dtb = create_fdt(
            FdtVmInfo::new(
                &mem,
                "console=tty0",
                None,
                FdtVcpuInfo::new(vcpu_id, vec![1; 128], vpmu_feature, true),
            ),
            FdtNumaInfo::new(cpu_maps, vec![], vec![], vec![0; 128]),
            FdtDeviceInfo::<MMIODeviceInfo>::new(None, gic.as_ref()),
        )
        .unwrap();

        create_dtb_file("output_with_cache.dtb", &dtb);

        let bytes = include_bytes!("test/output_with_cache.dtb");
        let pos = 4;
        let val = FDT_MAX_SIZE;
        let mut buf = vec![];
        buf.extend_from_slice(bytes);
        set_size(&mut buf, pos, val);

        let original_fdt = DeviceTree::load(&buf).unwrap();
        let generated_fdt = DeviceTree::load(&dtb).unwrap();
        assert!(format!("{original_fdt:?}") == format!("{generated_fdt:?}"));
    }

    #[test]
    fn test_create_fdt_with_numa() {
        let regions = arch_memory_regions(FDT_MAX_SIZE + 0x1000);
        let mem = GuestMemoryMmap::<()>::from_ranges(&regions).expect("Cannot initialize memory");
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let _ = vm.create_vcpu(0).unwrap();
        let _ = vm.create_vcpu(1).unwrap();
        let gic = create_gic(&vm, 2).unwrap();
        let vpmu_feature = VpmuFeatureLevel::Disabled;
        let dtb = create_fdt(
            FdtVmInfo::new(
                &mem,
                "console=tty0",
                None,
                FdtVcpuInfo::new(vec![0, 1], vec![1; 2], vpmu_feature, false),
            ),
            FdtNumaInfo::new(None, vec![1, 0], vec![1, 0], vec![]),
            FdtDeviceInfo::<MMIODeviceInfo>::new(None, gic.as_ref()),
        )
        .unwrap();

        create_dtb_file("output_with_numa.dtb", &dtb);

        let bytes = include_bytes!("test/output_with_numa.dtb");
        let pos = 4;
        let val = FDT_MAX_SIZE;
        let mut buf = vec![];
        buf.extend_from_slice(bytes);
        set_size(&mut buf, pos, val);

        let original_fdt = DeviceTree::load(&buf).unwrap();
        let generated_fdt = DeviceTree::load(&dtb).unwrap();
        assert_eq!(format!("{original_fdt:?}"), format!("{generated_fdt:?}"));
    }
}
