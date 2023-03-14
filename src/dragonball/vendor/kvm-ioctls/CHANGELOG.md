# v0.11.0

## Added
- [[#178](https://github.com/rust-vmm/kvm-ioctls/pull/178)] Support for the AMD
  Security Encrypted Virtualization (SEV) through the following VM ioctls:
  `encrypt_op`, `encrypt_op_sev`, `register_enc_memory_region` and
   `unregister_enc_memory_region`.
- [[#184](https://github.com/rust-vmm/kvm-ioctls/pull/184)] `DeviceFd` now
  derives `Debug`.

# v0.10.0

## Changed
- Now depends on kvm-bindings >=0.5.0 which replaced the v4.20 KVM bindings
  with the v5.13 ones.
- Updated `VcpuExit::Debug` to return architecture specific information for the
  debug event.

# v0.9.0

## Added
- Support for accessing and controlling the Time Stamp Counter on x86 platforms
  through the `get_tsc_khz` and `set_tsc_khz` functions.

## Changed
- Updated `create_vm` on `aarch64` to create a VM fd from the KVM fd using the
  host's maximum IPA size.

# v0.8.0

## Added
- Support for specifying VM type (an opaque platform and architecture specific
  constant) when creating a VM (`KVM_CREATE_VM` ioctl) via the
`Kvm::create_vm_with_type` function.

## Changed
- Now depends on kvm-bindings >=0.4.0 to support use of a newer vmm-sys-utils
  dependency.

# v0.7.0

## Added
- Support for the system API that returns the maximum allowed vCPU ID
  (`KVM_CAP_MAX_VCPU_ID`).
- Support for `KVM_MEMORY_ENCRYPT_OP`.

## Fixed
- [[#119](https://github.com/rust-vmm/kvm-ioctls/issues/119)]: Disallow invalid
  number of cpuid entries to be passed to `get_supported_cpuid` and
  `get_emulated_cpuid`.

## Changed
- [[#123](https://github.com/rust-vmm/kvm-ioctls/issues/123)]: Updated
  `create_vcpu` to use `u64` as the parameter for the number of vCPUs.

# v0.6.0

## Added
- Support for the vcpu ioctls: `KVM_SET_GUEST_DEBUG`, `KVM_KVMCLOCK_CTRL`, and
  `KVM_GET_REG_LIST`.
- Support for the vm ioctl `KVM_GET_DEVICE_ATTR`.
- Support for the device ioctl `KVM_HAS_DEVICE_ATTR`.
- Support for `VcpuExit::Debug`.
- Support for enabling vcpu capabilities using `Vcpu::enable_cap`.
- Support for checking Hyper-V (`HypervSynic` and `HypervSynic2`), MSI
  (`MsiDevid`), and IPA Size (`ArmVmIPASize`) capabilities.
  using `kvm.check_extension`.
- Support for checking the VM capabilities via `Vm::check_extension`.
- Create a VM with flexible IPA size using `Kvm::create_vm_with_ipa_size`.

## Removed
- Removed `Kvm::new_with_fd_number`. The same functionality is offered by the
  `Kvm` [FromRawFd](https://doc.rust-lang.org/std/os/unix/io/trait.FromRawFd.html)
  trait implementation.

## Changed
- The VM ioctl `unregister_ioevent` now correctly unregisters the events that
  correspond to the data match passed as a parameter.
- The `SystemEvent` Vcpu Exit now also contains the relevant type and flags.
- Updated `get_dirty_log` such that it does not assume the page size is 4K,
  but instead reads it using `libc::sysconf`.

# v0.5.0

## Added
- Support for the vcpu ioctls `KVM_GET/SET_VCPU_EVENTS` and `KVM_GET_DIRTY_LOG`
  on `aarch64`.
- Support for the vcpu ioctl `KVM_IRQ_LINE`.

# v0.4.0

## Added
- Support for unregistering ioeventfds through `KVM_IOEVENTFD`.

## Changed
- Functions working with event FDs now require
  vmm_sys_util::eventfd::EventFd in their interface instead of
  RawFd.
- Functions working with FAM structs kvm_msr_list and kvm_msrs, were
  changed to work with their respective safe counterparts MsrList and
  respectively Msrs.
- Now exporting kvm_ioctls::Error type definition so that users of this
  crate can create their own wrapping errors without having to know the
  Error type used internally by this crate.
- No longer exporting kvm_ioctls::Result. Users of this crate should
  not have to use kvm_ioctls::Result outside the crate.
- kvm_ioctls::Error now works with errno::Error instead of io::Error.

## Removed
- CpuId safe wrapper over FAM struct kvm_cpuid2. The safe wrapper is
  now provided by the kvm_bindings crate starting with v0.2.0.
- KVM_MAX_MSR_ENTRIES and MAX_KVM_CPUID_ENTRIES. Equivalent constants
  are provided by the kvm_bindings crate starting with v0.2.0.

# v0.3.0

## Added
- Support for setting vcpu `kvm_immediate_exit` flag
- Support for the vcpu ioctl `KVM_GET_CPUID2`
- Support for the vcpu ioctl `KVM_GET_MP_STATE`
- Support for the vcpu ioctl `KVM_SET_MP_STATE`
- Support for the vcpu ioctl `KVM_GET_VCPU_EVENTS`
- Support for the vcpu ioctl `KVM_SET_VCPU_EVENTS`
- Support for the vcpu ioctl `KVM_GET_DEBUGREGS`
- Support for the vcpu ioctl `KVM_SET_DEBUGREGS`
- Support for the vcpu ioctl `KVM_GET_XSAVE`
- Support for the vcpu ioctl `KVM_SET_XSAVE`
- Support for the vcpu ioctl `KVM_GET_XCRS`
- Support for the vcpu ioctl `KVM_SET_XCRS`
- Support for the vm ioctl `KVM_GET_IRQCHIP`
- Support for the vm ioctl `KVM_SET_IRQCHIP`
- Support for the vm ioctl `KVM_GET_CLOCK`
- Support for the vm ioctl `KVM_SET_CLOCK`
- Support for the vm ioctl `KVM_GET_PIT2`
- Support for the vm ioctl `KVM_SET_PIT2`
- Support for the vcpu ioctl `KVM_GET_ONE_REG`

## Changed
- Function offering support for `KVM_SET_MSRS` also returns the number
  of MSR entries successfully written.

# v0.2.0

## Added
- Add support for `KVM_ENABLE_CAP`.
- Add support for `KVM_SIGNAL_MSI`.

## Fixed
- Fix bug in KvmRunWrapper. The memory for kvm_run struct was not unmapped
  after the KvmRunWrapper object got out of scope.
- Return proper value when receiving the EOI KVM exit.
- Mark set_user_memory_region as unsafe.

# v0.1.0

First release of the kvm-ioctls crate.

The kvm-ioctls crate provides safe wrappers over the KVM API, a set of ioctls
used for creating and configuring Virtual Machines (VMs) on Linux.
The ioctls are accessible through four structures:
- Kvm - wrappers over system ioctls
- VmFd - wrappers over VM ioctls
- VcpuFd - wrappers over vCPU ioctls
- DeviceFd - wrappers over device ioctls

The kvm-ioctls can be used on x86_64 and aarch64. Right now the aarch64
support is considered experimental.
