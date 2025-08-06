# CPUID

## Design

CPUID is designed as the CPUID filter for Intel and AMD CPU Identification. Through CPUID configuration, we could set CPU topology, Cache topology, PMU status and other features for the VMs. 

CPUID is developed based on the Firecracker CPUID code while we add other extensions such as CPU Topology and VPMU features.

## Usage
To use CPUID, you should first use KVM_GET_CPUID2 ioctl to get the original CPUID then use process_cpuid() provided by the db-arch to filter CPUID with the information you want and suitable for VM conditions.

Currently, we support following specifications that db-arch could use to filter CPUID:
```rust
pub struct VmSpec {
    /// The vendor id of the CPU
    cpu_vendor_id: [u8; 12],
    /// The id of the current logical cpu in the range [0..cpu_count].
    cpu_id: u8,
    /// The total number of logical cpus (includes cpus that could be hotplugged).
    cpu_count: u8,
    /// The desired brand string for the guest.
    brand_string: BrandString,
    /// threads per core for cpu topology information
    threads_per_core: u8,
    /// cores per die for cpu topology information
    cores_per_die: u8,
    /// dies per socket for cpu topology information
    dies_per_socket: u8,
    /// if vpmu feature is Disabled, it means vpmu feature is off (by default)
    /// if vpmu feature is LimitedlyEnabled, it means minimal vpmu counters are supported (cycles and instructions)
    /// if vpmu feature is FullyEnabled, it means all vpmu counters are supported
    vpmu_feature: VpmuFeatureLevel,
}
```

## Example
We will show examples for filtering CPUID. 
First, you need to use KVM_GET_CPUID2 ioctl to get the original CPUID, this part is not included in the db-cpuid.

```rust
// an example for getting the cpuid in the vmm.
let mut cpuid = CpuId::new(num_entries).map_err(|_| errno::Error::new(libc::ENOMEM))?;
let ret = unsafe {ioctl_with_mut_ptr(self, KVM_GET_CPUID2(), cpuid.as_mut_fam_struct_ptr())};
if ret != 0 {
    return Err(errno::Error::last());
}
```

Then we could create the `VmSpec` to describe the VM specification we want and use process_cpuid() to filter CPUID.

```rust
let cpuid_vm_spec = VmSpec::new(
            self.id,
            vcpu_config.max_all_vcpu_count as u8,
            vcpu_config.threads_per_core,
            vcpu_config.cores_per_die,
            vcpu_config.dies_per_socket,
            vcpu_config.vpmu_feature,
        )
        .map_err(VcpuError::CpuId)?;
        process_cpuid(&mut self.cpuid, &cpuid_vm_spec).map_err(|e| {
            METRICS.vcpu.process_cpuid.inc();
            error!("Failure in configuring CPUID for vcpu {}: {:?}", self.id, e);
            VcpuError::CpuId(e)
        })?;
```

After the CPUID is filtered, we could use it to set the guest's CPUID.
