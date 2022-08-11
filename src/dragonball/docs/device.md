# Device

## Device Manager

Currently we have following device manager:
| Name | Description | 
| --- | --- |
| [address space manager](../src/address_space_manager.rs) | abstracts virtual machine's physical management and provide mapping for guest virtual memory and MMIO ranges of emulated virtual devices, pass-through devices and vCPU |
| [config manager](../src/config_manager.rs) | provides abstractions for configuration information | 
| [console manager](../src/device_manager/console_manager.rs) | provides management for all console devices | 
| [resource manager](../src/resource_manager.rs) |provides resource management for `legacy_irq_pool`, `msi_irq_pool`, `pio_pool`, `mmio_pool`, `mem_pool`, `kvm_mem_slot_pool` with builder `ResourceManagerBuilder` | 
| [VSOCK device manager](../src/device_manager/vsock_dev_mgr.rs) | provides configuration info for `VIRTIO-VSOCK` and management for all VSOCK devices | 
   

## Device supported
`VIRTIO-VSOCK`
`i8042`
`COM1`
`COM2`

