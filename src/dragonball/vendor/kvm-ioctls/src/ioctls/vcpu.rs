// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use kvm_bindings::*;
use libc::EINVAL;
use std::fs::File;
use std::os::unix::io::{AsRawFd, RawFd};

use ioctls::{KvmRunWrapper, Result};
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use kvm_bindings::{CpuId, Msrs, KVM_MAX_CPUID_ENTRIES};
use kvm_ioctls::*;
use vmm_sys_util::errno;
use vmm_sys_util::ioctl::{ioctl, ioctl_with_mut_ref, ioctl_with_ref};
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use vmm_sys_util::ioctl::{ioctl_with_mut_ptr, ioctl_with_ptr, ioctl_with_val};

/// Reasons for vCPU exits.
///
/// The exit reasons are mapped to the `KVM_EXIT_*` defines in the
/// [Linux KVM header](https://elixir.bootlin.com/linux/latest/source/include/uapi/linux/kvm.h).
#[derive(Debug)]
pub enum VcpuExit<'a> {
    /// An out port instruction was run on the given port with the given data.
    IoOut(u16 /* port */, &'a [u8] /* data */),
    /// An in port instruction was run on the given port.
    ///
    /// The given slice should be filled in before [run()](struct.VcpuFd.html#method.run)
    /// is called again.
    IoIn(u16 /* port */, &'a mut [u8] /* data */),
    /// A read instruction was run against the given MMIO address.
    ///
    /// The given slice should be filled in before [run()](struct.VcpuFd.html#method.run)
    /// is called again.
    MmioRead(u64 /* address */, &'a mut [u8]),
    /// A write instruction was run against the given MMIO address with the given data.
    MmioWrite(u64 /* address */, &'a [u8]),
    /// Corresponds to KVM_EXIT_UNKNOWN.
    Unknown,
    /// Corresponds to KVM_EXIT_EXCEPTION.
    Exception,
    /// Corresponds to KVM_EXIT_HYPERCALL.
    Hypercall,
    /// Corresponds to KVM_EXIT_DEBUG.
    ///
    /// Provides architecture specific information for the debug event.
    Debug(kvm_debug_exit_arch),
    /// Corresponds to KVM_EXIT_HLT.
    Hlt,
    /// Corresponds to KVM_EXIT_IRQ_WINDOW_OPEN.
    IrqWindowOpen,
    /// Corresponds to KVM_EXIT_SHUTDOWN.
    Shutdown,
    /// Corresponds to KVM_EXIT_FAIL_ENTRY.
    FailEntry,
    /// Corresponds to KVM_EXIT_INTR.
    Intr,
    /// Corresponds to KVM_EXIT_SET_TPR.
    SetTpr,
    /// Corresponds to KVM_EXIT_TPR_ACCESS.
    TprAccess,
    /// Corresponds to KVM_EXIT_S390_SIEIC.
    S390Sieic,
    /// Corresponds to KVM_EXIT_S390_RESET.
    S390Reset,
    /// Corresponds to KVM_EXIT_DCR.
    Dcr,
    /// Corresponds to KVM_EXIT_NMI.
    Nmi,
    /// Corresponds to KVM_EXIT_INTERNAL_ERROR.
    InternalError,
    /// Corresponds to KVM_EXIT_OSI.
    Osi,
    /// Corresponds to KVM_EXIT_PAPR_HCALL.
    PaprHcall,
    /// Corresponds to KVM_EXIT_S390_UCONTROL.
    S390Ucontrol,
    /// Corresponds to KVM_EXIT_WATCHDOG.
    Watchdog,
    /// Corresponds to KVM_EXIT_S390_TSCH.
    S390Tsch,
    /// Corresponds to KVM_EXIT_EPR.
    Epr,
    /// Corresponds to KVM_EXIT_SYSTEM_EVENT.
    SystemEvent(u32 /* type */, u64 /* flags */),
    /// Corresponds to KVM_EXIT_S390_STSI.
    S390Stsi,
    /// Corresponds to KVM_EXIT_IOAPIC_EOI.
    IoapicEoi(u8 /* vector */),
    /// Corresponds to KVM_EXIT_HYPERV.
    Hyperv,
}

/// Wrapper over KVM vCPU ioctls.
pub struct VcpuFd {
    vcpu: File,
    kvm_run_ptr: KvmRunWrapper,
}

impl VcpuFd {
    /// Returns the vCPU general purpose registers.
    ///
    /// The registers are returned in a `kvm_regs` structure as defined in the
    /// [KVM API documentation](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    /// See documentation for `KVM_GET_REGS`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    /// let regs = vcpu.get_regs().unwrap();
    /// ```
    #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn get_regs(&self) -> Result<kvm_regs> {
        // Safe because we know that our file is a vCPU fd, we know the kernel will only read the
        // correct amount of memory from our pointer, and we verify the return result.
        let mut regs = unsafe { std::mem::zeroed() };
        let ret = unsafe { ioctl_with_mut_ref(self, KVM_GET_REGS(), &mut regs) };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(regs)
    }

    /// Sets the vCPU general purpose registers using the `KVM_SET_REGS` ioctl.
    ///
    /// # Arguments
    ///
    /// * `regs` - general purpose registers. For details check the `kvm_regs` structure in the
    ///             [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    ///
    /// #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    /// {
    ///     // Get the current vCPU registers.
    ///     let mut regs = vcpu.get_regs().unwrap();
    ///     // Set a new value for the Instruction Pointer.
    ///     regs.rip = 0x100;
    ///     vcpu.set_regs(&regs).unwrap();
    /// }
    /// ```
    #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn set_regs(&self, regs: &kvm_regs) -> Result<()> {
        // Safe because we know that our file is a vCPU fd, we know the kernel will only read the
        // correct amount of memory from our pointer, and we verify the return result.
        let ret = unsafe { ioctl_with_ref(self, KVM_SET_REGS(), regs) };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Returns the vCPU special registers.
    ///
    /// The registers are returned in a `kvm_sregs` structure as defined in the
    /// [KVM API documentation](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    /// See documentation for `KVM_GET_SREGS`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    /// let sregs = vcpu.get_sregs().unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_sregs(&self) -> Result<kvm_sregs> {
        // Safe because we know that our file is a vCPU fd, we know the kernel will only write the
        // correct amount of memory to our pointer, and we verify the return result.
        let mut regs = kvm_sregs::default();

        let ret = unsafe { ioctl_with_mut_ref(self, KVM_GET_SREGS(), &mut regs) };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(regs)
    }

    /// Sets the vCPU special registers using the `KVM_SET_SREGS` ioctl.
    ///
    /// # Arguments
    ///
    /// * `sregs` - Special registers. For details check the `kvm_sregs` structure in the
    ///             [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    /// {
    ///     let mut sregs = vcpu.get_sregs().unwrap();
    ///     // Update the code segment (cs).
    ///     sregs.cs.base = 0;
    ///     sregs.cs.selector = 0;
    ///     vcpu.set_sregs(&sregs).unwrap();
    /// }
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_sregs(&self, sregs: &kvm_sregs) -> Result<()> {
        // Safe because we know that our file is a vCPU fd, we know the kernel will only read the
        // correct amount of memory from our pointer, and we verify the return result.
        let ret = unsafe { ioctl_with_ref(self, KVM_SET_SREGS(), sregs) };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Returns the floating point state (FPU) from the vCPU.
    ///
    /// The state is returned in a `kvm_fpu` structure as defined in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    /// See the documentation for `KVM_GET_FPU`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    /// let fpu = vcpu.get_fpu().unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_fpu(&self) -> Result<kvm_fpu> {
        let mut fpu = kvm_fpu::default();

        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_fpu struct.
            ioctl_with_mut_ref(self, KVM_GET_FPU(), &mut fpu)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(fpu)
    }

    /// Set the floating point state (FPU) of a vCPU using the `KVM_SET_FPU` ioct.
    ///
    /// # Arguments
    ///
    /// * `fpu` - FPU configuration. For details check the `kvm_fpu` structure in the
    ///           [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_ioctls::Kvm;
    /// # use kvm_bindings::kvm_fpu;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    /// {
    ///     let KVM_FPU_CWD: u16 = 0x37f;
    ///     let fpu = kvm_fpu {
    ///         fcw: KVM_FPU_CWD,
    ///         ..Default::default()
    ///     };
    ///     vcpu.set_fpu(&fpu).unwrap();
    /// }
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_fpu(&self, fpu: &kvm_fpu) -> Result<()> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_fpu struct.
            ioctl_with_ref(self, KVM_SET_FPU(), fpu)
        };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// X86 specific call to setup the CPUID registers.
    ///
    /// See the documentation for `KVM_SET_CPUID2`.
    ///
    /// # Arguments
    ///
    /// * `cpuid` - CPUID registers.
    ///
    /// # Example
    ///
    ///  ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_bindings::KVM_MAX_CPUID_ENTRIES;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let mut kvm_cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    ///
    /// // Update the CPUID entries to disable the EPB feature.
    /// const ECX_EPB_SHIFT: u32 = 3;
    /// {
    ///     let entries = kvm_cpuid.as_mut_slice();
    ///     for entry in entries.iter_mut() {
    ///         match entry.function {
    ///             6 => entry.ecx &= !(1 << ECX_EPB_SHIFT),
    ///             _ => (),
    ///         }
    ///     }
    /// }
    ///
    /// vcpu.set_cpuid2(&kvm_cpuid).unwrap();
    /// ```
    ///
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_cpuid2(&self, cpuid: &CpuId) -> Result<()> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_cpuid2 struct.
            ioctl_with_ptr(self, KVM_SET_CPUID2(), cpuid.as_fam_struct_ptr())
        };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// X86 specific call to retrieve the CPUID registers.
    ///
    /// It requires knowledge of how many `kvm_cpuid_entry2` entries there are to get.
    /// See the documentation for `KVM_GET_CPUID2` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `num_entries` - Number of CPUID entries to be read.
    ///
    /// # Example
    ///
    ///  ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_bindings::KVM_MAX_CPUID_ENTRIES;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let cpuid = vcpu.get_cpuid2(KVM_MAX_CPUID_ENTRIES).unwrap();
    /// ```
    ///
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_cpuid2(&self, num_entries: usize) -> Result<CpuId> {
        if num_entries > KVM_MAX_CPUID_ENTRIES {
            // Returns the same error the underlying `ioctl` would have sent.
            return Err(errno::Error::new(libc::ENOMEM));
        }

        let mut cpuid = CpuId::new(num_entries).map_err(|_| errno::Error::new(libc::ENOMEM))?;
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_cpuid2 struct.
            ioctl_with_mut_ptr(self, KVM_GET_CPUID2(), cpuid.as_mut_fam_struct_ptr())
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(cpuid)
    }

    ///
    /// See the documentation for `KVM_ENABLE_CAP`.
    ///
    /// # Arguments
    ///
    /// * kvm_enable_cap - KVM capability structure. For details check the `kvm_enable_cap`
    ///                    structure in the
    ///                    [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Example
    ///
    ///  ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_bindings::{kvm_enable_cap, KVM_MAX_CPUID_ENTRIES, KVM_CAP_HYPERV_SYNIC, KVM_CAP_SPLIT_IRQCHIP};
    /// # use kvm_ioctls::{Kvm, Cap};
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let mut cap: kvm_enable_cap = Default::default();
    /// if cfg!(target_arch = "x86") || cfg!(target_arch = "x86_64") {
    ///     // KVM_CAP_HYPERV_SYNIC needs KVM_CAP_SPLIT_IRQCHIP enabled
    ///     cap.cap = KVM_CAP_SPLIT_IRQCHIP;
    ///     cap.args[0] = 24;
    ///     vm.enable_cap(&cap).unwrap();
    ///
    ///     let vcpu = vm.create_vcpu(0).unwrap();
    ///     if kvm.check_extension(Cap::HypervSynic) {
    ///         let mut cap: kvm_enable_cap = Default::default();
    ///         cap.cap = KVM_CAP_HYPERV_SYNIC;
    ///         vcpu.enable_cap(&cap).unwrap();
    ///     }
    /// }
    /// ```
    ///
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn enable_cap(&self, cap: &kvm_enable_cap) -> Result<()> {
        // The ioctl is safe because we allocated the struct and we know the
        // kernel will write exactly the size of the struct.
        let ret = unsafe { ioctl_with_ref(self, KVM_ENABLE_CAP(), cap) };
        if ret == 0 {
            Ok(())
        } else {
            Err(errno::Error::last())
        }
    }

    /// Returns the state of the LAPIC (Local Advanced Programmable Interrupt Controller).
    ///
    /// The state is returned in a `kvm_lapic_state` structure as defined in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    /// See the documentation for `KVM_GET_LAPIC`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// // For `get_lapic` to work, you first need to create a IRQ chip before creating the vCPU.
    /// vm.create_irq_chip().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let lapic = vcpu.get_lapic().unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_lapic(&self) -> Result<kvm_lapic_state> {
        let mut klapic = kvm_lapic_state::default();

        let ret = unsafe {
            // The ioctl is unsafe unless you trust the kernel not to write past the end of the
            // local_apic struct.
            ioctl_with_mut_ref(self, KVM_GET_LAPIC(), &mut klapic)
        };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(klapic)
    }

    /// Sets the state of the LAPIC (Local Advanced Programmable Interrupt Controller).
    ///
    /// See the documentation for `KVM_SET_LAPIC`.
    ///
    /// # Arguments
    ///
    /// * `klapic` - LAPIC state. For details check the `kvm_lapic_state` structure in the
    ///             [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// use std::io::Write;
    ///
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// // For `get_lapic` to work, you first need to create a IRQ chip before creating the vCPU.
    /// vm.create_irq_chip().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let mut lapic = vcpu.get_lapic().unwrap();
    ///
    /// // Write to APIC_ICR offset the value 2.
    /// let apic_icr_offset = 0x300;
    /// let write_value: &[u8] = &[2, 0, 0, 0];
    /// let mut apic_icr_slice =
    ///     unsafe { &mut *(&mut lapic.regs[apic_icr_offset..] as *mut [i8] as *mut [u8]) };
    /// apic_icr_slice.write(write_value).unwrap();
    ///
    /// // Update the value of LAPIC.
    /// vcpu.set_lapic(&lapic).unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_lapic(&self, klapic: &kvm_lapic_state) -> Result<()> {
        let ret = unsafe {
            // The ioctl is safe because the kernel will only read from the klapic struct.
            ioctl_with_ref(self, KVM_SET_LAPIC(), klapic)
        };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Returns the model-specific registers (MSR) for this vCPU.
    ///
    /// It emulates `KVM_GET_MSRS` ioctl's behavior by returning the number of MSRs
    /// successfully read upon success or the last error number in case of failure.
    /// The MSRs are returned in the `msr` method argument.
    ///
    /// # Arguments
    ///
    /// * `msrs`  - MSRs (input/output). For details check the `kvm_msrs` structure in the
    ///             [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_ioctls::Kvm;
    /// # use kvm_bindings::{kvm_msr_entry, Msrs};
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// // Configure the struct to say which entries we want to get.
    /// let mut msrs = Msrs::from_entries(&[
    ///     kvm_msr_entry {
    ///         index: 0x0000_0174,
    ///         ..Default::default()
    ///     },
    ///     kvm_msr_entry {
    ///         index: 0x0000_0175,
    ///         ..Default::default()
    ///     },
    /// ])
    /// .unwrap();
    /// let read = vcpu.get_msrs(&mut msrs).unwrap();
    /// assert_eq!(read, 2);
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_msrs(&self, msrs: &mut Msrs) -> Result<usize> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_msrs struct.
            ioctl_with_mut_ptr(self, KVM_GET_MSRS(), msrs.as_mut_fam_struct_ptr())
        };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(ret as usize)
    }

    /// Setup the model-specific registers (MSR) for this vCPU.
    /// Returns the number of MSR entries actually written.
    ///
    /// See the documentation for `KVM_SET_MSRS`.
    ///
    /// # Arguments
    ///
    /// * `msrs` - MSRs. For details check the `kvm_msrs` structure in the
    ///            [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_ioctls::Kvm;
    /// # use kvm_bindings::{kvm_msr_entry, Msrs};
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    ///
    /// // Configure the entries we want to set.
    /// let mut msrs = Msrs::from_entries(&[kvm_msr_entry {
    ///     index: 0x0000_0174,
    ///     ..Default::default()
    /// }])
    /// .unwrap();
    /// let written = vcpu.set_msrs(&msrs).unwrap();
    /// assert_eq!(written, 1);
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_msrs(&self, msrs: &Msrs) -> Result<usize> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_msrs struct.
            ioctl_with_ptr(self, KVM_SET_MSRS(), msrs.as_fam_struct_ptr())
        };
        // KVM_SET_MSRS actually returns the number of msr entries written.
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(ret as usize)
    }

    /// Returns the vcpu's current "multiprocessing state".
    ///
    /// See the documentation for `KVM_GET_MP_STATE` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_mp_state` - multiprocessing state to be read.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let mp_state = vcpu.get_mp_state().unwrap();
    /// ```
    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "s390"
    ))]
    pub fn get_mp_state(&self) -> Result<kvm_mp_state> {
        let mut mp_state = Default::default();
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_mp_state struct.
            ioctl_with_mut_ref(self, KVM_GET_MP_STATE(), &mut mp_state)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(mp_state)
    }

    /// Sets the vcpu's current "multiprocessing state".
    ///
    /// See the documentation for `KVM_SET_MP_STATE` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_mp_state` - multiprocessing state to be written.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let mp_state = Default::default();
    /// // Your `mp_state` manipulation here.
    /// vcpu.set_mp_state(mp_state).unwrap();
    /// ```
    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "s390"
    ))]
    pub fn set_mp_state(&self, mp_state: kvm_mp_state) -> Result<()> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_mp_state struct.
            ioctl_with_ref(self, KVM_SET_MP_STATE(), &mp_state)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// X86 specific call that returns the vcpu's current "xsave struct".
    ///
    /// See the documentation for `KVM_GET_XSAVE` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_xsave` - xsave struct to be read.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let xsave = vcpu.get_xsave().unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_xsave(&self) -> Result<kvm_xsave> {
        let mut xsave = Default::default();
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_xsave struct.
            ioctl_with_mut_ref(self, KVM_GET_XSAVE(), &mut xsave)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(xsave)
    }

    /// X86 specific call that sets the vcpu's current "xsave struct".
    ///
    /// See the documentation for `KVM_SET_XSAVE` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_xsave` - xsave struct to be written.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let xsave = Default::default();
    /// // Your `xsave` manipulation here.
    /// vcpu.set_xsave(&xsave).unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_xsave(&self, xsave: &kvm_xsave) -> Result<()> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_xsave struct.
            ioctl_with_ref(self, KVM_SET_XSAVE(), xsave)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// X86 specific call that returns the vcpu's current "xcrs".
    ///
    /// See the documentation for `KVM_GET_XCRS` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_xcrs` - xcrs to be read.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let xcrs = vcpu.get_xcrs().unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_xcrs(&self) -> Result<kvm_xcrs> {
        let mut xcrs = Default::default();
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_xcrs struct.
            ioctl_with_mut_ref(self, KVM_GET_XCRS(), &mut xcrs)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(xcrs)
    }

    /// X86 specific call that sets the vcpu's current "xcrs".
    ///
    /// See the documentation for `KVM_SET_XCRS` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_xcrs` - xcrs to be written.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let xcrs = Default::default();
    /// // Your `xcrs` manipulation here.
    /// vcpu.set_xcrs(&xcrs).unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_xcrs(&self, xcrs: &kvm_xcrs) -> Result<()> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_xcrs struct.
            ioctl_with_ref(self, KVM_SET_XCRS(), xcrs)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// X86 specific call that returns the vcpu's current "debug registers".
    ///
    /// See the documentation for `KVM_GET_DEBUGREGS` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_debugregs` - debug registers to be read.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let debug_regs = vcpu.get_debug_regs().unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_debug_regs(&self) -> Result<kvm_debugregs> {
        let mut debug_regs = Default::default();
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_debugregs struct.
            ioctl_with_mut_ref(self, KVM_GET_DEBUGREGS(), &mut debug_regs)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(debug_regs)
    }

    /// X86 specific call that sets the vcpu's current "debug registers".
    ///
    /// See the documentation for `KVM_SET_DEBUGREGS` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_debugregs` - debug registers to be written.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let debug_regs = Default::default();
    /// // Your `debug_regs` manipulation here.
    /// vcpu.set_debug_regs(&debug_regs).unwrap();
    /// ```
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_debug_regs(&self, debug_regs: &kvm_debugregs) -> Result<()> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_debugregs struct.
            ioctl_with_ref(self, KVM_SET_DEBUGREGS(), debug_regs)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Returns currently pending exceptions, interrupts, and NMIs as well as related
    /// states of the vcpu.
    ///
    /// See the documentation for `KVM_GET_VCPU_EVENTS` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_vcpu_events` - vcpu events to be read.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::{Kvm, Cap};
    /// let kvm = Kvm::new().unwrap();
    /// if kvm.check_extension(Cap::VcpuEvents) {
    ///     let vm = kvm.create_vm().unwrap();
    ///     let vcpu = vm.create_vcpu(0).unwrap();
    ///     let vcpu_events = vcpu.get_vcpu_events().unwrap();
    /// }
    /// ```
    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64"
    ))]
    pub fn get_vcpu_events(&self) -> Result<kvm_vcpu_events> {
        let mut vcpu_events = Default::default();
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_vcpu_events struct.
            ioctl_with_mut_ref(self, KVM_GET_VCPU_EVENTS(), &mut vcpu_events)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(vcpu_events)
    }

    /// Sets pending exceptions, interrupts, and NMIs as well as related states of the vcpu.
    ///
    /// See the documentation for `KVM_SET_VCPU_EVENTS` in the
    /// [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Arguments
    ///
    /// * `kvm_vcpu_events` - vcpu events to be written.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::{Kvm, Cap};
    /// let kvm = Kvm::new().unwrap();
    /// if kvm.check_extension(Cap::VcpuEvents) {
    ///     let vm = kvm.create_vm().unwrap();
    ///     let vcpu = vm.create_vcpu(0).unwrap();
    ///     let vcpu_events = Default::default();
    ///     // Your `vcpu_events` manipulation here.
    ///     vcpu.set_vcpu_events(&vcpu_events).unwrap();
    /// }
    /// ```
    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64"
    ))]

    pub fn set_vcpu_events(&self, vcpu_events: &kvm_vcpu_events) -> Result<()> {
        let ret = unsafe {
            // Here we trust the kernel not to read past the end of the kvm_vcpu_events struct.
            ioctl_with_ref(self, KVM_SET_VCPU_EVENTS(), vcpu_events)
        };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Sets the type of CPU to be exposed to the guest and optional features.
    ///
    /// This initializes an ARM vCPU to the specified type with the specified features
    /// and resets the values of all of its registers to defaults. See the documentation for
    /// `KVM_ARM_VCPU_INIT`.
    ///
    /// # Arguments
    ///
    /// * `kvi` - information about preferred CPU target type and recommended features for it.
    ///           For details check the `kvm_vcpu_init` structure in the
    ///           [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Example
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_ioctls::Kvm;
    /// use kvm_bindings::kvm_vcpu_init;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    ///
    /// let mut kvi = kvm_vcpu_init::default();
    /// vm.get_preferred_target(&mut kvi).unwrap();
    /// vcpu.vcpu_init(&kvi).unwrap();
    /// ```
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    pub fn vcpu_init(&self, kvi: &kvm_vcpu_init) -> Result<()> {
        // This is safe because we allocated the struct and we know the kernel will read
        // exactly the size of the struct.
        let ret = unsafe { ioctl_with_ref(self, KVM_ARM_VCPU_INIT(), kvi) };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Returns the guest registers that are supported for the
    /// KVM_GET_ONE_REG/KVM_SET_ONE_REG calls.
    ///
    /// # Arguments
    ///
    /// * `reg_list`  - list of registers (input/output). For details check the `kvm_reg_list`
    ///                 structure in the
    ///                 [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_ioctls::Kvm;
    /// # use kvm_bindings::RegList;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    ///
    /// // KVM_GET_REG_LIST demands that the vcpus be initalized.
    /// let mut kvi: kvm_bindings::kvm_vcpu_init = kvm_bindings::kvm_vcpu_init::default();
    /// vm.get_preferred_target(&mut kvi).unwrap();
    /// vcpu.vcpu_init(&kvi).expect("Cannot initialize vcpu");
    ///
    /// let mut reg_list = RegList::new(500).unwrap();
    /// vcpu.get_reg_list(&mut reg_list).unwrap();
    /// assert!(reg_list.as_fam_struct_ref().n > 0);
    /// ```
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    pub fn get_reg_list(&self, reg_list: &mut RegList) -> Result<()> {
        let ret =
            unsafe { ioctl_with_mut_ref(self, KVM_GET_REG_LIST(), reg_list.as_mut_fam_struct()) };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Sets processor-specific debug registers and configures the vcpu for handling
    /// certain guest debug events using the `KVM_SET_GUEST_DEBUG` ioctl.
    ///
    /// # Arguments
    ///
    /// * `debug_struct` - control bitfields and debug registers, depending on the specific architecture.
    ///             For details check the `kvm_guest_debug` structure in the
    ///             [KVM API doc](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_ioctls::Kvm;
    /// # use kvm_bindings::{
    /// #     KVM_GUESTDBG_ENABLE, KVM_GUESTDBG_USE_SW_BP, kvm_guest_debug_arch, kvm_guest_debug
    /// # };
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    ///
    /// #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    /// {
    ///     let debug_struct = kvm_guest_debug {
    ///         // Configure the vcpu so that a KVM_DEBUG_EXIT would be generated
    ///         // when encountering a software breakpoint during execution
    ///         control: KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_USE_SW_BP,
    ///         pad: 0,
    ///         // Reset all x86-specific debug registers
    ///         arch: kvm_guest_debug_arch {
    ///             debugreg: [0, 0, 0, 0, 0, 0, 0, 0],
    ///         },
    ///     };
    ///
    ///     vcpu.set_guest_debug(&debug_struct).unwrap();
    /// }
    /// ```
    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm64",
        target_arch = "s390",
        target_arch = "ppc"
    ))]
    pub fn set_guest_debug(&self, debug_struct: &kvm_guest_debug) -> Result<()> {
        let ret = unsafe { ioctl_with_ref(self, KVM_SET_GUEST_DEBUG(), debug_struct) };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Sets the value of one register for this vCPU.
    ///
    /// The id of the register is encoded as specified in the kernel documentation
    /// for `KVM_SET_ONE_REG`.
    ///
    /// # Arguments
    ///
    /// * `reg_id` - ID of the register for which we are setting the value.
    /// * `data` - value for the specified register.
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    pub fn set_one_reg(&self, reg_id: u64, data: u64) -> Result<()> {
        let data_ref = &data as *const u64;
        let onereg = kvm_one_reg {
            id: reg_id,
            addr: data_ref as u64,
        };
        // This is safe because we allocated the struct and we know the kernel will read
        // exactly the size of the struct.
        let ret = unsafe { ioctl_with_ref(self, KVM_SET_ONE_REG(), &onereg) };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Returns the value of the specified vCPU register.
    ///
    /// The id of the register is encoded as specified in the kernel documentation
    /// for `KVM_GET_ONE_REG`.
    ///
    /// # Arguments
    ///
    /// * `reg_id` - ID of the register.
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    pub fn get_one_reg(&self, reg_id: u64) -> Result<u64> {
        let mut reg_value = 0;
        let mut onereg = kvm_one_reg {
            id: reg_id,
            addr: &mut reg_value as *mut u64 as u64,
        };

        let ret = unsafe { ioctl_with_mut_ref(self, KVM_GET_ONE_REG(), &mut onereg) };
        if ret < 0 {
            return Err(errno::Error::last());
        }
        Ok(reg_value)
    }

    /// Notify the guest about the vCPU being paused.
    ///
    /// See the documentation for `KVM_KVMCLOCK_CTRL` in the
    /// [KVM API documentation](https://www.kernel.org/doc/Documentation/virtual/kvm/api.txt).
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn kvmclock_ctrl(&self) -> Result<()> {
        // Safe because we know that our file is a KVM fd and that the request
        // is one of the ones defined by kernel.
        let ret = unsafe { ioctl(self, KVM_KVMCLOCK_CTRL()) };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Triggers the running of the current virtual CPU returning an exit reason.
    ///
    /// See documentation for `KVM_RUN`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use std::io::Write;
    /// # use std::ptr::null_mut;
    /// # use std::slice;
    /// # use kvm_ioctls::{Kvm, VcpuExit};
    /// # use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
    /// # let kvm = Kvm::new().unwrap();
    /// # let vm = kvm.create_vm().unwrap();
    /// // This is a dummy example for running on x86 based on https://lwn.net/Articles/658511/.
    /// #[cfg(target_arch = "x86_64")]
    /// {
    ///     let mem_size = 0x4000;
    ///     let guest_addr: u64 = 0x1000;
    ///     let load_addr: *mut u8 = unsafe {
    ///         libc::mmap(
    ///             null_mut(),
    ///             mem_size,
    ///             libc::PROT_READ | libc::PROT_WRITE,
    ///             libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
    ///             -1,
    ///             0,
    ///         ) as *mut u8
    ///     };
    ///
    ///     let mem_region = kvm_userspace_memory_region {
    ///         slot: 0,
    ///         guest_phys_addr: guest_addr,
    ///         memory_size: mem_size as u64,
    ///         userspace_addr: load_addr as u64,
    ///         flags: 0,
    ///     };
    ///     unsafe { vm.set_user_memory_region(mem_region).unwrap() };
    ///
    ///     // Dummy x86 code that just calls halt.
    ///     let x86_code = [0xf4 /* hlt */];
    ///
    ///     // Write the code in the guest memory. This will generate a dirty page.
    ///     unsafe {
    ///         let mut slice = slice::from_raw_parts_mut(load_addr, mem_size);
    ///         slice.write(&x86_code).unwrap();
    ///     }
    ///
    ///     let vcpu_fd = vm.create_vcpu(0).unwrap();
    ///
    ///     let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
    ///     vcpu_sregs.cs.base = 0;
    ///     vcpu_sregs.cs.selector = 0;
    ///     vcpu_fd.set_sregs(&vcpu_sregs).unwrap();
    ///
    ///     let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    ///     // Set the Instruction Pointer to the guest address where we loaded the code.
    ///     vcpu_regs.rip = guest_addr;
    ///     vcpu_regs.rax = 2;
    ///     vcpu_regs.rbx = 3;
    ///     vcpu_regs.rflags = 2;
    ///     vcpu_fd.set_regs(&vcpu_regs).unwrap();
    ///
    ///     loop {
    ///         match vcpu_fd.run().expect("run failed") {
    ///             VcpuExit::Hlt => {
    ///                 break;
    ///             }
    ///             exit_reason => panic!("unexpected exit reason: {:?}", exit_reason),
    ///         }
    ///     }
    /// }
    /// ```
    pub fn run(&self) -> Result<VcpuExit> {
        // Safe because we know that our file is a vCPU fd and we verify the return result.
        let ret = unsafe { ioctl(self, KVM_RUN()) };
        if ret == 0 {
            let run = self.kvm_run_ptr.as_mut_ref();
            match run.exit_reason {
                // make sure you treat all possible exit reasons from include/uapi/linux/kvm.h corresponding
                // when upgrading to a different kernel version
                KVM_EXIT_UNKNOWN => Ok(VcpuExit::Unknown),
                KVM_EXIT_EXCEPTION => Ok(VcpuExit::Exception),
                KVM_EXIT_IO => {
                    let run_start = run as *mut kvm_run as *mut u8;
                    // Safe because the exit_reason (which comes from the kernel) told us which
                    // union field to use.
                    let io = unsafe { run.__bindgen_anon_1.io };
                    let port = io.port;
                    let data_size = io.count as usize * io.size as usize;
                    // The data_offset is defined by the kernel to be some number of bytes into the
                    // kvm_run stucture, which we have fully mmap'd.
                    let data_ptr = unsafe { run_start.offset(io.data_offset as isize) };
                    // The slice's lifetime is limited to the lifetime of this vCPU, which is equal
                    // to the mmap of the `kvm_run` struct that this is slicing from.
                    let data_slice = unsafe {
                        std::slice::from_raw_parts_mut::<u8>(data_ptr as *mut u8, data_size)
                    };
                    match u32::from(io.direction) {
                        KVM_EXIT_IO_IN => Ok(VcpuExit::IoIn(port, data_slice)),
                        KVM_EXIT_IO_OUT => Ok(VcpuExit::IoOut(port, data_slice)),
                        _ => Err(errno::Error::new(EINVAL)),
                    }
                }
                KVM_EXIT_HYPERCALL => Ok(VcpuExit::Hypercall),
                KVM_EXIT_DEBUG => {
                    // Safe because the exit_reason (which comes from the kernel) told us which
                    // union field to use.
                    let debug = unsafe { run.__bindgen_anon_1.debug };
                    Ok(VcpuExit::Debug(debug.arch))
                }
                KVM_EXIT_HLT => Ok(VcpuExit::Hlt),
                KVM_EXIT_MMIO => {
                    // Safe because the exit_reason (which comes from the kernel) told us which
                    // union field to use.
                    let mmio = unsafe { &mut run.__bindgen_anon_1.mmio };
                    let addr = mmio.phys_addr;
                    let len = mmio.len as usize;
                    let data_slice = &mut mmio.data[..len];
                    if mmio.is_write != 0 {
                        Ok(VcpuExit::MmioWrite(addr, data_slice))
                    } else {
                        Ok(VcpuExit::MmioRead(addr, data_slice))
                    }
                }
                KVM_EXIT_IRQ_WINDOW_OPEN => Ok(VcpuExit::IrqWindowOpen),
                KVM_EXIT_SHUTDOWN => Ok(VcpuExit::Shutdown),
                KVM_EXIT_FAIL_ENTRY => Ok(VcpuExit::FailEntry),
                KVM_EXIT_INTR => Ok(VcpuExit::Intr),
                KVM_EXIT_SET_TPR => Ok(VcpuExit::SetTpr),
                KVM_EXIT_TPR_ACCESS => Ok(VcpuExit::TprAccess),
                KVM_EXIT_S390_SIEIC => Ok(VcpuExit::S390Sieic),
                KVM_EXIT_S390_RESET => Ok(VcpuExit::S390Reset),
                KVM_EXIT_DCR => Ok(VcpuExit::Dcr),
                KVM_EXIT_NMI => Ok(VcpuExit::Nmi),
                KVM_EXIT_INTERNAL_ERROR => Ok(VcpuExit::InternalError),
                KVM_EXIT_OSI => Ok(VcpuExit::Osi),
                KVM_EXIT_PAPR_HCALL => Ok(VcpuExit::PaprHcall),
                KVM_EXIT_S390_UCONTROL => Ok(VcpuExit::S390Ucontrol),
                KVM_EXIT_WATCHDOG => Ok(VcpuExit::Watchdog),
                KVM_EXIT_S390_TSCH => Ok(VcpuExit::S390Tsch),
                KVM_EXIT_EPR => Ok(VcpuExit::Epr),
                KVM_EXIT_SYSTEM_EVENT => {
                    // Safe because the exit_reason (which comes from the kernel) told us which
                    // union field to use.
                    let system_event = unsafe { &mut run.__bindgen_anon_1.system_event };
                    Ok(VcpuExit::SystemEvent(
                        system_event.type_,
                        system_event.flags,
                    ))
                }
                KVM_EXIT_S390_STSI => Ok(VcpuExit::S390Stsi),
                KVM_EXIT_IOAPIC_EOI => {
                    // Safe because the exit_reason (which comes from the kernel) told us which
                    // union field to use.
                    let eoi = unsafe { &mut run.__bindgen_anon_1.eoi };
                    Ok(VcpuExit::IoapicEoi(eoi.vector))
                }
                KVM_EXIT_HYPERV => Ok(VcpuExit::Hyperv),
                r => panic!("unknown kvm exit reason: {}", r),
            }
        } else {
            Err(errno::Error::last())
        }
    }

    /// Sets the `immediate_exit` flag on the `kvm_run` struct associated with this vCPU to `val`.
    pub fn set_kvm_immediate_exit(&self, val: u8) {
        let kvm_run = self.kvm_run_ptr.as_mut_ref();
        kvm_run.immediate_exit = val;
    }

    /// Returns the vCPU TSC frequency in KHz or an error if the host has unstable TSC.
    ///
    /// # Example
    ///
    ///  ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// let tsc_khz = vcpu.get_tsc_khz().unwrap();
    /// ```
    ///
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get_tsc_khz(&self) -> Result<u32> {
        // Safe because we know that our file is a KVM fd and that the request is one of the ones
        // defined by kernel.
        let ret = unsafe { ioctl(self, KVM_GET_TSC_KHZ()) };
        if ret >= 0 {
            Ok(ret as u32)
        } else {
            Err(errno::Error::new(ret))
        }
    }

    /// Sets the specified vCPU TSC frequency.
    ///
    /// # Arguments
    ///
    /// * `freq` - The frequency unit is KHz as per the KVM API documentation
    /// for `KVM_SET_TSC_KHZ`.
    ///
    /// # Example
    ///
    ///  ```rust
    /// # extern crate kvm_ioctls;
    /// # use kvm_ioctls::{Cap, Kvm};
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    /// let vcpu = vm.create_vcpu(0).unwrap();
    /// if kvm.check_extension(Cap::GetTscKhz) && kvm.check_extension(Cap::TscControl) {
    ///     vcpu.set_tsc_khz(1000).unwrap();
    /// }
    /// ```
    ///
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set_tsc_khz(&self, freq: u32) -> Result<()> {
        // Safe because we know that our file is a KVM fd and that the request is one of the ones
        // defined by kernel.
        let ret = unsafe { ioctl_with_val(self, KVM_SET_TSC_KHZ(), freq as u64) };
        if ret < 0 {
            Err(errno::Error::last())
        } else {
            Ok(())
        }
    }
}

/// Helper function to create a new `VcpuFd`.
///
/// This should not be exported as a public function because the preferred way is to use
/// `create_vcpu` from `VmFd`. The function cannot be part of the `VcpuFd` implementation because
/// then it would be exported with the public `VcpuFd` interface.
pub fn new_vcpu(vcpu: File, kvm_run_ptr: KvmRunWrapper) -> VcpuFd {
    VcpuFd { vcpu, kvm_run_ptr }
}

impl AsRawFd for VcpuFd {
    fn as_raw_fd(&self) -> RawFd {
        self.vcpu.as_raw_fd()
    }
}

#[cfg(test)]
mod tests {
    extern crate byteorder;

    use super::*;
    use ioctls::system::Kvm;
    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64"
    ))]
    use Cap;

    // Helper function for memory mapping `size` bytes of anonymous memory.
    // Panics if the mmap fails.
    fn mmap_anonymous(size: usize) -> *mut u8 {
        use std::ptr::null_mut;

        let addr = unsafe {
            libc::mmap(
                null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
                -1,
                0,
            )
        };
        if addr == libc::MAP_FAILED {
            panic!("mmap failed.");
        }

        addr as *mut u8
    }

    #[test]
    fn test_create_vcpu() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();

        assert!(vm.create_vcpu(0).is_ok());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_get_cpuid() {
        let kvm = Kvm::new().unwrap();
        if kvm.check_extension(Cap::ExtCpuid) {
            let vm = kvm.create_vm().unwrap();
            let cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();
            let ncpuids = cpuid.as_slice().len();
            assert!(ncpuids <= KVM_MAX_CPUID_ENTRIES);
            let nr_vcpus = kvm.get_nr_vcpus();
            for cpu_idx in 0..nr_vcpus {
                let vcpu = vm.create_vcpu(cpu_idx as u64).unwrap();
                vcpu.set_cpuid2(&cpuid).unwrap();
                let retrieved_cpuid = vcpu.get_cpuid2(ncpuids).unwrap();
                // Only check the first few leafs as some (e.g. 13) are reserved.
                assert_eq!(cpuid.as_slice()[..3], retrieved_cpuid.as_slice()[..3]);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_get_cpuid_fail_num_entries_too_high() {
        let kvm = Kvm::new().unwrap();
        if kvm.check_extension(Cap::ExtCpuid) {
            let vm = kvm.create_vm().unwrap();
            let vcpu = vm.create_vcpu(0).unwrap();
            let err_cpuid = vcpu.get_cpuid2(KVM_MAX_CPUID_ENTRIES + 1_usize).err();
            assert_eq!(err_cpuid.unwrap().errno(), libc::ENOMEM);
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_get_cpuid_fail_num_entries_too_small() {
        let kvm = Kvm::new().unwrap();
        if kvm.check_extension(Cap::ExtCpuid) {
            let vm = kvm.create_vm().unwrap();
            let cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();
            let ncpuids = cpuid.as_slice().len();
            assert!(ncpuids <= KVM_MAX_CPUID_ENTRIES);
            let nr_vcpus = kvm.get_nr_vcpus();
            for cpu_idx in 0..nr_vcpus {
                let vcpu = vm.create_vcpu(cpu_idx as u64).unwrap();
                vcpu.set_cpuid2(&cpuid).unwrap();
                let err = vcpu.get_cpuid2(ncpuids - 1_usize).err();
                assert_eq!(err.unwrap().errno(), libc::E2BIG);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_set_cpuid() {
        let kvm = Kvm::new().unwrap();
        if kvm.check_extension(Cap::ExtCpuid) {
            let vm = kvm.create_vm().unwrap();
            let mut cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();
            let ncpuids = cpuid.as_slice().len();
            assert!(ncpuids <= KVM_MAX_CPUID_ENTRIES);
            let vcpu = vm.create_vcpu(0).unwrap();

            // Setting Manufacturer ID
            {
                let entries = cpuid.as_mut_slice();
                for entry in entries.iter_mut() {
                    if entry.function == 0 {
                        // " KVMKVMKVM "
                        entry.ebx = 0x4b4d564b;
                        entry.ecx = 0x564b4d56;
                        entry.edx = 0x4d;
                    }
                }
            }
            vcpu.set_cpuid2(&cpuid).unwrap();
            let cpuid_0 = vcpu.get_cpuid2(ncpuids).unwrap();
            for entry in cpuid_0.as_slice() {
                if entry.function == 0 {
                    assert_eq!(entry.ebx, 0x4b4d564b);
                    assert_eq!(entry.ecx, 0x564b4d56);
                    assert_eq!(entry.edx, 0x4d);
                }
            }

            // Disabling Intel SHA extensions.
            const EBX_SHA_SHIFT: u32 = 29;
            let mut ebx_sha_off = 0u32;
            {
                let entries = cpuid.as_mut_slice();
                for entry in entries.iter_mut() {
                    if entry.function == 7 && entry.ecx == 0 {
                        entry.ebx &= !(1 << EBX_SHA_SHIFT);
                        ebx_sha_off = entry.ebx;
                    }
                }
            }
            vcpu.set_cpuid2(&cpuid).unwrap();
            let cpuid_1 = vcpu.get_cpuid2(ncpuids).unwrap();
            for entry in cpuid_1.as_slice() {
                if entry.function == 7 && entry.ecx == 0 {
                    assert_eq!(entry.ebx, ebx_sha_off);
                }
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[allow(non_snake_case)]
    #[test]
    fn test_fpu() {
        // as per https://github.com/torvalds/linux/blob/master/arch/x86/include/asm/fpu/internal.h
        let KVM_FPU_CWD: usize = 0x37f;
        let KVM_FPU_MXCSR: usize = 0x1f80;
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let mut fpu: kvm_fpu = kvm_fpu {
            fcw: KVM_FPU_CWD as u16,
            mxcsr: KVM_FPU_MXCSR as u32,
            ..Default::default()
        };

        fpu.fcw = KVM_FPU_CWD as u16;
        fpu.mxcsr = KVM_FPU_MXCSR as u32;

        vcpu.set_fpu(&fpu).unwrap();
        assert_eq!(vcpu.get_fpu().unwrap().fcw, KVM_FPU_CWD as u16);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn lapic_test() {
        use std::io::Cursor;
        // We might get read of byteorder if we replace mem::transmute with something safer.
        use self::byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
        // As per https://github.com/torvalds/linux/arch/x86/kvm/lapic.c
        // Try to write and read the APIC_ICR (0x300) register which is non-read only and
        // one can simply write to it.
        let kvm = Kvm::new().unwrap();
        assert!(kvm.check_extension(Cap::Irqchip));
        let vm = kvm.create_vm().unwrap();
        // The get_lapic ioctl will fail if there is no irqchip created beforehand.
        assert!(vm.create_irq_chip().is_ok());
        let vcpu = vm.create_vcpu(0).unwrap();
        let mut klapic: kvm_lapic_state = vcpu.get_lapic().unwrap();

        let reg_offset = 0x300;
        let value = 2_u32;
        //try to write and read the APIC_ICR	0x300
        let write_slice =
            unsafe { &mut *(&mut klapic.regs[reg_offset..] as *mut [i8] as *mut [u8]) };
        let mut writer = Cursor::new(write_slice);
        writer.write_u32::<LittleEndian>(value).unwrap();
        vcpu.set_lapic(&klapic).unwrap();
        klapic = vcpu.get_lapic().unwrap();
        let read_slice = unsafe { &*(&klapic.regs[reg_offset..] as *const [i8] as *const [u8]) };
        let mut reader = Cursor::new(read_slice);
        assert_eq!(reader.read_u32::<LittleEndian>().unwrap(), value);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn msrs_test() {
        use vmm_sys_util::fam::FamStruct;
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();

        // Set the following MSRs.
        let msrs_to_set = [
            kvm_msr_entry {
                index: 0x0000_0174,
                data: 0x0,
                ..Default::default()
            },
            kvm_msr_entry {
                index: 0x0000_0175,
                data: 0x1,
                ..Default::default()
            },
        ];
        let msrs_wrapper = Msrs::from_entries(&msrs_to_set).unwrap();
        vcpu.set_msrs(&msrs_wrapper).unwrap();

        // Now test that GET_MSRS returns the same.
        // Configure the struct to say which entries we want.
        let mut returned_kvm_msrs = Msrs::from_entries(&[
            kvm_msr_entry {
                index: 0x0000_0174,
                ..Default::default()
            },
            kvm_msr_entry {
                index: 0x0000_0175,
                ..Default::default()
            },
        ])
        .unwrap();
        let nmsrs = vcpu.get_msrs(&mut returned_kvm_msrs).unwrap();

        // Verify the lengths match.
        assert_eq!(nmsrs, msrs_to_set.len());
        assert_eq!(nmsrs, returned_kvm_msrs.as_fam_struct_ref().len() as usize);

        // Verify the contents match.
        let returned_kvm_msr_entries = returned_kvm_msrs.as_slice();
        for (i, entry) in returned_kvm_msr_entries.iter().enumerate() {
            assert_eq!(entry, &msrs_to_set[i]);
        }
    }

    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "s390"
    ))]
    #[test]
    fn mpstate_test() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let mp_state = vcpu.get_mp_state().unwrap();
        vcpu.set_mp_state(mp_state).unwrap();
        let other_mp_state = vcpu.get_mp_state().unwrap();
        assert_eq!(mp_state, other_mp_state);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn xsave_test() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let xsave = vcpu.get_xsave().unwrap();
        vcpu.set_xsave(&xsave).unwrap();
        let other_xsave = vcpu.get_xsave().unwrap();
        assert_eq!(&xsave.region[..], &other_xsave.region[..]);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn xcrs_test() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let xcrs = vcpu.get_xcrs().unwrap();
        vcpu.set_xcrs(&xcrs).unwrap();
        let other_xcrs = vcpu.get_xcrs().unwrap();
        assert_eq!(xcrs, other_xcrs);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn debugregs_test() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let debugregs = vcpu.get_debug_regs().unwrap();
        vcpu.set_debug_regs(&debugregs).unwrap();
        let other_debugregs = vcpu.get_debug_regs().unwrap();
        assert_eq!(debugregs, other_debugregs);
    }

    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64"
    ))]
    #[test]
    fn vcpu_events_test() {
        let kvm = Kvm::new().unwrap();
        if kvm.check_extension(Cap::VcpuEvents) {
            let vm = kvm.create_vm().unwrap();
            let vcpu = vm.create_vcpu(0).unwrap();
            let vcpu_events = vcpu.get_vcpu_events().unwrap();
            vcpu.set_vcpu_events(&vcpu_events).unwrap();
            let other_vcpu_events = vcpu.get_vcpu_events().unwrap();
            assert_eq!(vcpu_events, other_vcpu_events);
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn test_run_code() {
        use std::io::Write;

        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        #[rustfmt::skip]
        let code = [
            0x40, 0x20, 0x80, 0x52, /* mov w0, #0x102 */
            0x00, 0x01, 0x00, 0xb9, /* str w0, [x8]; test physical memory write */
            0x81, 0x60, 0x80, 0x52, /* mov w1, #0x304 */
            0x02, 0x00, 0x80, 0x52, /* mov w2, #0x0 */
            0x20, 0x01, 0x40, 0xb9, /* ldr w0, [x9]; test MMIO read */
            0x1f, 0x18, 0x14, 0x71, /* cmp w0, #0x506 */
            0x20, 0x00, 0x82, 0x1a, /* csel w0, w1, w2, eq */
            0x20, 0x01, 0x00, 0xb9, /* str w0, [x9]; test MMIO write */
            0x00, 0x80, 0xb0, 0x52, /* mov w0, #0x84000000 */
            0x00, 0x00, 0x1d, 0x32, /* orr w0, w0, #0x08 */
            0x02, 0x00, 0x00, 0xd4, /* hvc #0x0 */
            0x00, 0x00, 0x00, 0x14, /* b <this address>; shouldn't get here, but if so loop forever */
        ];

        let mem_size = 0x20000;
        let load_addr = mmap_anonymous(mem_size);
        let guest_addr: u64 = 0x10000;
        let slot: u32 = 0;
        let mem_region = kvm_userspace_memory_region {
            slot,
            guest_phys_addr: guest_addr,
            memory_size: mem_size as u64,
            userspace_addr: load_addr as u64,
            flags: KVM_MEM_LOG_DIRTY_PAGES,
        };
        unsafe {
            vm.set_user_memory_region(mem_region).unwrap();
        }

        unsafe {
            // Get a mutable slice of `mem_size` from `load_addr`.
            // This is safe because we mapped it before.
            let mut slice = std::slice::from_raw_parts_mut(load_addr, mem_size);
            slice.write_all(&code).unwrap();
        }

        let vcpu_fd = vm.create_vcpu(0).unwrap();
        let mut kvi = kvm_bindings::kvm_vcpu_init::default();
        vm.get_preferred_target(&mut kvi).unwrap();
        kvi.features[0] |= 1 << KVM_ARM_VCPU_PSCI_0_2;
        vcpu_fd.vcpu_init(&kvi).unwrap();

        let core_reg_base: u64 = 0x6030_0000_0010_0000;
        let mmio_addr: u64 = guest_addr + mem_size as u64;

        // Set the PC to the guest address where we loaded the code.
        vcpu_fd
            .set_one_reg(core_reg_base + 2 * 32, guest_addr)
            .unwrap();

        // Set x8 and x9 to the addresses the guest test code needs
        vcpu_fd
            .set_one_reg(core_reg_base + 2 * 8, guest_addr + 0x10000)
            .unwrap();
        vcpu_fd
            .set_one_reg(core_reg_base + 2 * 9, mmio_addr)
            .unwrap();

        loop {
            match vcpu_fd.run().expect("run failed") {
                VcpuExit::MmioRead(addr, data) => {
                    assert_eq!(addr, mmio_addr);
                    assert_eq!(data.len(), 4);
                    data[3] = 0x0;
                    data[2] = 0x0;
                    data[1] = 0x5;
                    data[0] = 0x6;
                }
                VcpuExit::MmioWrite(addr, data) => {
                    assert_eq!(addr, mmio_addr);
                    assert_eq!(data.len(), 4);
                    assert_eq!(data[3], 0x0);
                    assert_eq!(data[2], 0x0);
                    assert_eq!(data[1], 0x3);
                    assert_eq!(data[0], 0x4);
                    // The code snippet dirties one page at guest_addr + 0x10000.
                    // The code page should not be dirty, as it's not written by the guest.
                    let dirty_pages_bitmap = vm.get_dirty_log(slot, mem_size).unwrap();
                    let dirty_pages: u32 = dirty_pages_bitmap
                        .into_iter()
                        .map(|page| page.count_ones())
                        .sum();
                    assert_eq!(dirty_pages, 1);
                }
                VcpuExit::SystemEvent(type_, flags) => {
                    assert_eq!(type_, KVM_SYSTEM_EVENT_SHUTDOWN);
                    assert_eq!(flags, 0);
                    break;
                }
                r => panic!("unexpected exit reason: {:?}", r),
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_run_code() {
        use std::io::Write;

        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        // This example is based on https://lwn.net/Articles/658511/
        #[rustfmt::skip]
        let code = [
            0xba, 0xf8, 0x03, /* mov $0x3f8, %dx */
            0x00, 0xd8, /* add %bl, %al */
            0x04, b'0', /* add $'0', %al */
            0xee, /* out %al, %dx */
            0xec, /* in %dx, %al */
            0xc6, 0x06, 0x00, 0x80, 0x00, /* movl $0, (0x8000); This generates a MMIO Write.*/
            0x8a, 0x16, 0x00, 0x80, /* movl (0x8000), %dl; This generates a MMIO Read.*/
            0xc6, 0x06, 0x00, 0x20, 0x00, /* movl $0, (0x2000); Dirty one page in guest mem. */
            0xf4, /* hlt */
        ];
        let expected_rips: [u64; 3] = [0x1003, 0x1005, 0x1007];

        let mem_size = 0x4000;
        let load_addr = mmap_anonymous(mem_size);
        let guest_addr: u64 = 0x1000;
        let slot: u32 = 0;
        let mem_region = kvm_userspace_memory_region {
            slot,
            guest_phys_addr: guest_addr,
            memory_size: mem_size as u64,
            userspace_addr: load_addr as u64,
            flags: KVM_MEM_LOG_DIRTY_PAGES,
        };
        unsafe {
            vm.set_user_memory_region(mem_region).unwrap();
        }

        unsafe {
            // Get a mutable slice of `mem_size` from `load_addr`.
            // This is safe because we mapped it before.
            let mut slice = std::slice::from_raw_parts_mut(load_addr, mem_size);
            slice.write_all(&code).unwrap();
        }

        let vcpu_fd = vm.create_vcpu(0).unwrap();

        let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
        assert_ne!(vcpu_sregs.cs.base, 0);
        assert_ne!(vcpu_sregs.cs.selector, 0);
        vcpu_sregs.cs.base = 0;
        vcpu_sregs.cs.selector = 0;
        vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

        let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
        // Set the Instruction Pointer to the guest address where we loaded the code.
        vcpu_regs.rip = guest_addr;
        vcpu_regs.rax = 2;
        vcpu_regs.rbx = 3;
        vcpu_regs.rflags = 2;
        vcpu_fd.set_regs(&vcpu_regs).unwrap();

        let mut debug_struct = kvm_guest_debug {
            control: KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_SINGLESTEP,
            pad: 0,
            arch: kvm_guest_debug_arch {
                debugreg: [0, 0, 0, 0, 0, 0, 0, 0],
            },
        };
        vcpu_fd.set_guest_debug(&debug_struct).unwrap();

        let mut instr_idx = 0;
        loop {
            match vcpu_fd.run().expect("run failed") {
                VcpuExit::IoIn(addr, data) => {
                    assert_eq!(addr, 0x3f8);
                    assert_eq!(data.len(), 1);
                }
                VcpuExit::IoOut(addr, data) => {
                    assert_eq!(addr, 0x3f8);
                    assert_eq!(data.len(), 1);
                    assert_eq!(data[0], b'5');
                }
                VcpuExit::MmioRead(addr, data) => {
                    assert_eq!(addr, 0x8000);
                    assert_eq!(data.len(), 1);
                }
                VcpuExit::MmioWrite(addr, data) => {
                    assert_eq!(addr, 0x8000);
                    assert_eq!(data.len(), 1);
                    assert_eq!(data[0], 0);
                }
                VcpuExit::Debug(debug) => {
                    if instr_idx == expected_rips.len() - 1 {
                        // Disabling debugging/single-stepping
                        debug_struct.control = 0;
                        vcpu_fd.set_guest_debug(&debug_struct).unwrap();
                    } else if instr_idx >= expected_rips.len() {
                        unreachable!();
                    }
                    let vcpu_regs = vcpu_fd.get_regs().unwrap();
                    assert_eq!(vcpu_regs.rip, expected_rips[instr_idx]);
                    assert_eq!(debug.exception, 1);
                    assert_eq!(debug.pc, expected_rips[instr_idx]);
                    // Check first 15 bits of DR6
                    let mask = (1 << 16) - 1;
                    assert_eq!(debug.dr6 & mask, 0b100111111110000);
                    // Bit 10 in DR7 is always 1
                    assert_eq!(debug.dr7, 1 << 10);
                    instr_idx += 1;
                }
                VcpuExit::Hlt => {
                    // The code snippet dirties 2 pages:
                    // * one when the code itself is loaded in memory;
                    // * and one more from the `movl` that writes to address 0x8000
                    let dirty_pages_bitmap = vm.get_dirty_log(slot, mem_size).unwrap();
                    let dirty_pages: u32 = dirty_pages_bitmap
                        .into_iter()
                        .map(|page| page.count_ones())
                        .sum();
                    assert_eq!(dirty_pages, 2);
                    break;
                }
                r => panic!("unexpected exit reason: {:?}", r),
            }
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_faulty_vcpu_fd() {
        use std::os::unix::io::FromRawFd;

        let badf_errno = libc::EBADF;

        let faulty_vcpu_fd = VcpuFd {
            vcpu: unsafe { File::from_raw_fd(-2) },
            kvm_run_ptr: KvmRunWrapper {
                kvm_run_ptr: mmap_anonymous(10),
                mmap_size: 10,
            },
        };

        assert_eq!(faulty_vcpu_fd.get_regs().unwrap_err().errno(), badf_errno);
        assert_eq!(
            faulty_vcpu_fd
                .set_regs(&unsafe { std::mem::zeroed() })
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(faulty_vcpu_fd.get_sregs().unwrap_err().errno(), badf_errno);
        assert_eq!(
            faulty_vcpu_fd
                .set_sregs(&unsafe { std::mem::zeroed() })
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(faulty_vcpu_fd.get_fpu().unwrap_err().errno(), badf_errno);
        assert_eq!(
            faulty_vcpu_fd
                .set_fpu(&unsafe { std::mem::zeroed() })
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd
                .set_cpuid2(
                    &Kvm::new()
                        .unwrap()
                        .get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)
                        .unwrap()
                )
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd.get_cpuid2(1).err().unwrap().errno(),
            badf_errno
        );
        // `kvm_lapic_state` does not implement debug by default so we cannot
        // use unwrap_err here.
        assert!(faulty_vcpu_fd.get_lapic().is_err());
        assert_eq!(
            faulty_vcpu_fd
                .set_lapic(&unsafe { std::mem::zeroed() })
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd
                .get_msrs(&mut Msrs::new(1).unwrap())
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd
                .set_msrs(&Msrs::new(1).unwrap())
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd.get_mp_state().unwrap_err().errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd
                .set_mp_state(kvm_mp_state::default())
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd.get_xsave().err().unwrap().errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd
                .set_xsave(&kvm_xsave::default())
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(faulty_vcpu_fd.get_xcrs().unwrap_err().errno(), badf_errno);
        assert_eq!(
            faulty_vcpu_fd
                .set_xcrs(&kvm_xcrs::default())
                .err()
                .unwrap()
                .errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd.get_debug_regs().unwrap_err().errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd
                .set_debug_regs(&kvm_debugregs::default())
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd.get_vcpu_events().unwrap_err().errno(),
            badf_errno
        );
        assert_eq!(
            faulty_vcpu_fd
                .set_vcpu_events(&kvm_vcpu_events::default())
                .unwrap_err()
                .errno(),
            badf_errno
        );
        assert_eq!(faulty_vcpu_fd.run().unwrap_err().errno(), badf_errno);
        assert_eq!(
            faulty_vcpu_fd.kvmclock_ctrl().unwrap_err().errno(),
            badf_errno
        );
        assert!(faulty_vcpu_fd.get_tsc_khz().is_err());
        assert!(faulty_vcpu_fd.set_tsc_khz(1000000).is_err());
    }

    #[test]
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    fn test_get_preferred_target() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();

        let mut kvi: kvm_bindings::kvm_vcpu_init = kvm_bindings::kvm_vcpu_init::default();
        assert!(vcpu.vcpu_init(&kvi).is_err());

        vm.get_preferred_target(&mut kvi)
            .expect("Cannot get preferred target");
        assert!(vcpu.vcpu_init(&kvi).is_ok());
    }

    #[test]
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    fn test_set_one_reg() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();

        let mut kvi: kvm_bindings::kvm_vcpu_init = kvm_bindings::kvm_vcpu_init::default();
        vm.get_preferred_target(&mut kvi)
            .expect("Cannot get preferred target");
        vcpu.vcpu_init(&kvi).expect("Cannot initialize vcpu");
        let data: u64 = 0;
        let reg_id: u64 = 0;

        assert!(vcpu.set_one_reg(reg_id, data).is_err());
        // Exercising KVM_SET_ONE_REG by trying to alter the data inside the PSTATE register (which is a
        // specific aarch64 register).
        const PSTATE_REG_ID: u64 = 0x6030_0000_0010_0042;
        vcpu.set_one_reg(PSTATE_REG_ID, data)
            .expect("Failed to set pstate register");
    }

    #[test]
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    fn test_get_one_reg() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();

        let mut kvi: kvm_bindings::kvm_vcpu_init = kvm_bindings::kvm_vcpu_init::default();
        vm.get_preferred_target(&mut kvi)
            .expect("Cannot get preferred target");
        vcpu.vcpu_init(&kvi).expect("Cannot initialize vcpu");

        // PSR (Processor State Register) bits.
        // Taken from arch/arm64/include/uapi/asm/ptrace.h.
        const PSR_MODE_EL1H: u64 = 0x0000_0005;
        const PSR_F_BIT: u64 = 0x0000_0040;
        const PSR_I_BIT: u64 = 0x0000_0080;
        const PSR_A_BIT: u64 = 0x0000_0100;
        const PSR_D_BIT: u64 = 0x0000_0200;
        const PSTATE_FAULT_BITS_64: u64 =
            PSR_MODE_EL1H | PSR_A_BIT | PSR_F_BIT | PSR_I_BIT | PSR_D_BIT;
        let data: u64 = PSTATE_FAULT_BITS_64;
        const PSTATE_REG_ID: u64 = 0x6030_0000_0010_0042;
        vcpu.set_one_reg(PSTATE_REG_ID, data)
            .expect("Failed to set pstate register");

        assert_eq!(
            vcpu.get_one_reg(PSTATE_REG_ID)
                .expect("Failed to get pstate register"),
            PSTATE_FAULT_BITS_64
        );
    }

    #[test]
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    fn test_get_reg_list() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();

        let mut reg_list = RegList::new(1).unwrap();
        // KVM_GET_REG_LIST demands that the vcpus be initalized, so we expect this to fail.
        let err = vcpu.get_reg_list(&mut reg_list).unwrap_err();
        assert!(err.errno() == libc::ENOEXEC);

        let mut kvi: kvm_bindings::kvm_vcpu_init = kvm_bindings::kvm_vcpu_init::default();
        vm.get_preferred_target(&mut kvi)
            .expect("Cannot get preferred target");
        vcpu.vcpu_init(&kvi).expect("Cannot initialize vcpu");

        // KVM_GET_REG_LIST offers us a number of registers for which we have
        // not allocated memory, so the first time it fails.
        let err = vcpu.get_reg_list(&mut reg_list).unwrap_err();
        assert!(err.errno() == libc::E2BIG);
        assert!(reg_list.as_mut_fam_struct().n > 0);

        // We make use of the number of registers returned to allocate memory and
        // try one more time.
        let mut reg_list = RegList::new(reg_list.as_mut_fam_struct().n as usize).unwrap();
        assert!(vcpu.get_reg_list(&mut reg_list).is_ok());
    }

    #[test]
    fn set_kvm_immediate_exit() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        assert_eq!(vcpu.kvm_run_ptr.as_mut_ref().immediate_exit, 0);
        vcpu.set_kvm_immediate_exit(1);
        assert_eq!(vcpu.kvm_run_ptr.as_mut_ref().immediate_exit, 1);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_enable_cap() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let mut cap = kvm_enable_cap {
            // KVM_CAP_HYPERV_SYNIC needs KVM_CAP_SPLIT_IRQCHIP enabled
            cap: KVM_CAP_SPLIT_IRQCHIP,
            ..Default::default()
        };
        cap.args[0] = 24;
        vm.enable_cap(&cap).unwrap();

        let vcpu = vm.create_vcpu(0).unwrap();
        if kvm.check_extension(Cap::HypervSynic) {
            let cap = kvm_enable_cap {
                cap: KVM_CAP_HYPERV_SYNIC,
                ..Default::default()
            };
            vcpu.enable_cap(&cap).unwrap();
        }
    }
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_get_tsc_khz() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();

        if !kvm.check_extension(Cap::GetTscKhz) {
            assert!(vcpu.get_tsc_khz().is_err())
        } else {
            assert!(vcpu.get_tsc_khz().unwrap() > 0);
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_set_tsc_khz() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let vcpu = vm.create_vcpu(0).unwrap();
        let freq = vcpu.get_tsc_khz().unwrap();

        if !(kvm.check_extension(Cap::GetTscKhz) && kvm.check_extension(Cap::TscControl)) {
            assert!(vcpu.set_tsc_khz(0).is_err());
        } else {
            assert!(vcpu.set_tsc_khz(freq - 500000).is_ok());
            assert_eq!(vcpu.get_tsc_khz().unwrap(), freq - 500000);
            assert!(vcpu.set_tsc_khz(freq + 500000).is_ok());
            assert_eq!(vcpu.get_tsc_khz().unwrap(), freq + 500000);
        }
    }
}
