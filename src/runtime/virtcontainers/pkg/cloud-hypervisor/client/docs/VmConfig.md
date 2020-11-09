# VmConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Cpus** | [**CpusConfig**](CpusConfig.md) |  | [optional] 
**Memory** | [**MemoryConfig**](MemoryConfig.md) |  | [optional] 
**Kernel** | [**KernelConfig**](KernelConfig.md) |  | 
**Initramfs** | Pointer to [**InitramfsConfig**](InitramfsConfig.md) |  | [optional] 
**Cmdline** | [**CmdLineConfig**](CmdLineConfig.md) |  | [optional] 
**Disks** | [**[]DiskConfig**](DiskConfig.md) |  | [optional] 
**Net** | [**[]NetConfig**](NetConfig.md) |  | [optional] 
**Rng** | [**RngConfig**](RngConfig.md) |  | [optional] 
**Balloon** | [**BalloonConfig**](BalloonConfig.md) |  | [optional] 
**Fs** | [**[]FsConfig**](FsConfig.md) |  | [optional] 
**Pmem** | [**[]PmemConfig**](PmemConfig.md) |  | [optional] 
**Serial** | [**ConsoleConfig**](ConsoleConfig.md) |  | [optional] 
**Console** | [**ConsoleConfig**](ConsoleConfig.md) |  | [optional] 
**Devices** | [**[]DeviceConfig**](DeviceConfig.md) |  | [optional] 
**Vsock** | [**VsockConfig**](VsockConfig.md) |  | [optional] 
**SgxEpc** | [**[]SgxEpcConfig**](SgxEpcConfig.md) |  | [optional] 
**Numa** | [**[]NumaConfig**](NumaConfig.md) |  | [optional] 
**Iommu** | **bool** |  | [optional] [default to false]
**Watchdog** | **bool** |  | [optional] [default to false]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


