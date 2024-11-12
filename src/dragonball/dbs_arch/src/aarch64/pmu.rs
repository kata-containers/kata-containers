// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constants and utilities for aarch64 PMU virtualization.

use kvm_bindings::{
    kvm_device_attr, KVM_ARM_VCPU_PMU_V3_CTRL, KVM_ARM_VCPU_PMU_V3_INIT, KVM_ARM_VCPU_PMU_V3_IRQ,
};
use kvm_ioctls::{Error as KvmError, VcpuFd, VmFd};
use thiserror::Error;

/// PPI base number on aarch64.
pub const PPI_BASE: u32 = 16;
/// Pmu ppi number
pub const VIRTUAL_PMU_IRQ: u32 = 7;

/// Errors thrown while setting up the PMU.
#[derive(Error, Debug)]
pub enum PmuError {
    /// Error while check kvm pmu capability
    #[error("Check kvm pmu capability failed: {0}")]
    CheckKvmPmuCap(#[source] KvmError),
    /// Error while check pmu irq.
    #[error("Check pmu irq error: {0}")]
    HasPmuIrq(#[source] KvmError),
    /// Error while check pmu init.
    #[error("Check pmu init error: {0}")]
    HasPmuInit(#[source] KvmError),
    /// Error while set pmu irq.
    #[error("Set pmu irq error: {0}")]
    SetPmuIrq(#[source] KvmError),
    /// Error while set pmu init.
    #[error("Set pmu init error: {0}")]
    SetPmuInit(#[source] KvmError),
}

type Result<T> = std::result::Result<T, PmuError>;

/// Tests whether a cpu supports KVM_ARM_VCPU_PMU_V3_IRQ attribute.
///
/// # Arguments
/// * `vcpu` - The VCPU file descriptor
fn has_pmu_irq(vcpu: &VcpuFd) -> Result<()> {
    let irq = (VIRTUAL_PMU_IRQ + PPI_BASE) as u64;
    let attribute = kvm_device_attr {
        group: KVM_ARM_VCPU_PMU_V3_CTRL,
        attr: u64::from(KVM_ARM_VCPU_PMU_V3_IRQ),
        addr: &irq as *const u64 as u64,
        flags: 0,
    };
    vcpu.has_device_attr(&attribute)
        .map_err(PmuError::HasPmuIrq)
}

/// Tests whether a cpu supports KVM_ARM_VCPU_PMU_V3_INIT attribute.
///
/// # Arguments
/// * `vcpu` - The VCPU file descriptor
fn has_pmu_init(vcpu: &VcpuFd) -> Result<()> {
    let attribute = kvm_device_attr {
        group: KVM_ARM_VCPU_PMU_V3_CTRL,
        attr: u64::from(KVM_ARM_VCPU_PMU_V3_INIT),
        addr: 0,
        flags: 0,
    };
    vcpu.has_device_attr(&attribute)
        .map_err(PmuError::HasPmuInit)
}

/// Set KVM_ARM_VCPU_PMU_V3_IRQ for a specific vcpu.
///
/// # Arguments
/// * `vcpu` - The VCPU file descriptor
fn set_pmu_irq(vcpu: &VcpuFd) -> Result<()> {
    let irq = (VIRTUAL_PMU_IRQ + PPI_BASE) as u64;
    let attribute = kvm_device_attr {
        group: KVM_ARM_VCPU_PMU_V3_CTRL,
        attr: u64::from(KVM_ARM_VCPU_PMU_V3_IRQ),
        addr: &irq as *const u64 as u64,
        flags: 0,
    };
    vcpu.set_device_attr(&attribute)
        .map_err(PmuError::SetPmuIrq)
}

/// Set KVM_ARM_VCPU_PMU_V3_INIT for a specific vcpu.
///
/// # Arguments
/// * `vcpu` - The VCPU file descriptor
fn set_pmu_init(vcpu: &VcpuFd) -> Result<()> {
    let attribute = kvm_device_attr {
        group: KVM_ARM_VCPU_PMU_V3_CTRL,
        attr: u64::from(KVM_ARM_VCPU_PMU_V3_INIT),
        addr: 0,
        flags: 0,
    };
    vcpu.set_device_attr(&attribute)
        .map_err(PmuError::SetPmuInit)
}

/// Check kvm pmu capability
///
/// # Arguments
/// * `vm` - The VM file descriptor
fn check_kvm_pmu_cap(_vm: &VmFd) -> Result<()> {
    // TODO: check KVM_CAP_ARM_PMU_V3 capability before setting PMU
    // Cap for KVM_CAP_ARM_PMU_V3 isn't supported in kvm-ioctls upstream, so
    // leave a todo here for supporting this check in the future.
    // Interface: vm.check_extension(kvm_ioctls::Cap)

    Ok(())
}

/// Check pmu feature
///
/// # Arguments
/// * `vcpu` - The VCPU file descriptor
fn check_pmu_feature(vcpu: &VcpuFd) -> Result<()> {
    has_pmu_irq(vcpu)?;
    has_pmu_init(vcpu)
}

/// Set pmu feature
///
/// # Arguments
/// * `vcpu` - The VCPU file descriptor
fn set_pmu_feature(vcpu: &VcpuFd) -> Result<()> {
    set_pmu_irq(vcpu)?;
    set_pmu_init(vcpu)
}

/// Initialize PMU in for vcpu
///
/// # Arguments
/// * `vm` - The VM file descriptor
/// * `vcpu` - The VCPU file descriptor
pub fn initialize_pmu(vm: &VmFd, vcpu: &VcpuFd) -> Result<()> {
    check_kvm_pmu_cap(vm)?;
    check_pmu_feature(vcpu)?;
    set_pmu_feature(vcpu)
}

#[cfg(test)]
mod tests {
    use kvm_bindings::{kvm_vcpu_init, KVM_ARM_VCPU_PMU_V3, KVM_ARM_VCPU_PSCI_0_2};
    use kvm_ioctls::Kvm;

    use super::*;
    use crate::gic::create_gic;

    #[test]
    fn test_create_pmu() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();

        assert!(create_gic(&vm, 1).is_ok());
        assert!(initialize_pmu(&vm, &vcpu).is_err());

        if check_kvm_pmu_cap(&vm).is_err() {
            return;
        }

        let mut kvi: kvm_vcpu_init = kvm_vcpu_init::default();
        vm.get_preferred_target(&mut kvi)
            .expect("Cannot get preferred target");
        kvi.features[0] = 1 << KVM_ARM_VCPU_PSCI_0_2 | 1 << KVM_ARM_VCPU_PMU_V3;

        assert!(vcpu.vcpu_init(&kvi).is_ok());
        assert!(initialize_pmu(&vm, &vcpu).is_ok());
    }
}
