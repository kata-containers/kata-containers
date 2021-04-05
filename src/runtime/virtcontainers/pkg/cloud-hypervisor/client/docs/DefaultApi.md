# \DefaultApi

All URIs are relative to *http://localhost/api/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**BootVM**](DefaultApi.md#BootVM) | **Put** /vm.boot | Boot the previously created VM instance.
[**CreateVM**](DefaultApi.md#CreateVM) | **Put** /vm.create | Create the cloud-hypervisor Virtual Machine (VM) instance. The instance is not booted, only created.
[**DeleteVM**](DefaultApi.md#DeleteVM) | **Put** /vm.delete | Delete the cloud-hypervisor Virtual Machine (VM) instance.
[**PauseVM**](DefaultApi.md#PauseVM) | **Put** /vm.pause | Pause a previously booted VM instance.
[**PowerButtonVM**](DefaultApi.md#PowerButtonVM) | **Put** /vm.power-button | Trigger a power button in the VM
[**RebootVM**](DefaultApi.md#RebootVM) | **Put** /vm.reboot | Reboot the VM instance.
[**ResumeVM**](DefaultApi.md#ResumeVM) | **Put** /vm.resume | Resume a previously paused VM instance.
[**ShutdownVM**](DefaultApi.md#ShutdownVM) | **Put** /vm.shutdown | Shut the VM instance down.
[**ShutdownVMM**](DefaultApi.md#ShutdownVMM) | **Put** /vmm.shutdown | Shuts the cloud-hypervisor VMM.
[**VmAddDevicePut**](DefaultApi.md#VmAddDevicePut) | **Put** /vm.add-device | Add a new device to the VM
[**VmAddDiskPut**](DefaultApi.md#VmAddDiskPut) | **Put** /vm.add-disk | Add a new disk to the VM
[**VmAddFsPut**](DefaultApi.md#VmAddFsPut) | **Put** /vm.add-fs | Add a new virtio-fs device to the VM
[**VmAddNetPut**](DefaultApi.md#VmAddNetPut) | **Put** /vm.add-net | Add a new network device to the VM
[**VmAddPmemPut**](DefaultApi.md#VmAddPmemPut) | **Put** /vm.add-pmem | Add a new pmem device to the VM
[**VmAddVsockPut**](DefaultApi.md#VmAddVsockPut) | **Put** /vm.add-vsock | Add a new vsock device to the VM
[**VmCountersGet**](DefaultApi.md#VmCountersGet) | **Get** /vm.counters | Get counters from the VM
[**VmInfoGet**](DefaultApi.md#VmInfoGet) | **Get** /vm.info | Returns general information about the cloud-hypervisor Virtual Machine (VM) instance.
[**VmRemoveDevicePut**](DefaultApi.md#VmRemoveDevicePut) | **Put** /vm.remove-device | Remove a device from the VM
[**VmResizePut**](DefaultApi.md#VmResizePut) | **Put** /vm.resize | Resize the VM
[**VmResizeZonePut**](DefaultApi.md#VmResizeZonePut) | **Put** /vm.resize-zone | Resize a memory zone
[**VmRestorePut**](DefaultApi.md#VmRestorePut) | **Put** /vm.restore | Restore a VM from a snapshot.
[**VmSnapshotPut**](DefaultApi.md#VmSnapshotPut) | **Put** /vm.snapshot | Returns a VM snapshot.
[**VmmPingGet**](DefaultApi.md#VmmPingGet) | **Get** /vmm.ping | Ping the VMM to check for API server availability



## BootVM

> BootVM(ctx, )

Boot the previously created VM instance.

### Required Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## CreateVM

> CreateVM(ctx, vmConfig)

Create the cloud-hypervisor Virtual Machine (VM) instance. The instance is not booted, only created.

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**vmConfig** | [**VmConfig**](VmConfig.md)| The VM configuration | 

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## DeleteVM

> DeleteVM(ctx, )

Delete the cloud-hypervisor Virtual Machine (VM) instance.

### Required Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## PauseVM

> PauseVM(ctx, )

Pause a previously booted VM instance.

### Required Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## PowerButtonVM

> PowerButtonVM(ctx, )

Trigger a power button in the VM

### Required Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## RebootVM

> RebootVM(ctx, )

Reboot the VM instance.

### Required Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## ResumeVM

> ResumeVM(ctx, )

Resume a previously paused VM instance.

### Required Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## ShutdownVM

> ShutdownVM(ctx, )

Shut the VM instance down.

### Required Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## ShutdownVMM

> ShutdownVMM(ctx, )

Shuts the cloud-hypervisor VMM.

### Required Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmAddDevicePut

> PciDeviceInfo VmAddDevicePut(ctx, vmAddDevice)

Add a new device to the VM

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**vmAddDevice** | [**VmAddDevice**](VmAddDevice.md)| The path of the new device | 

### Return type

[**PciDeviceInfo**](PciDeviceInfo.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmAddDiskPut

> PciDeviceInfo VmAddDiskPut(ctx, diskConfig)

Add a new disk to the VM

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**diskConfig** | [**DiskConfig**](DiskConfig.md)| The details of the new disk | 

### Return type

[**PciDeviceInfo**](PciDeviceInfo.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmAddFsPut

> PciDeviceInfo VmAddFsPut(ctx, fsConfig)

Add a new virtio-fs device to the VM

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**fsConfig** | [**FsConfig**](FsConfig.md)| The details of the new virtio-fs | 

### Return type

[**PciDeviceInfo**](PciDeviceInfo.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmAddNetPut

> PciDeviceInfo VmAddNetPut(ctx, netConfig)

Add a new network device to the VM

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**netConfig** | [**NetConfig**](NetConfig.md)| The details of the new network device | 

### Return type

[**PciDeviceInfo**](PciDeviceInfo.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmAddPmemPut

> PciDeviceInfo VmAddPmemPut(ctx, pmemConfig)

Add a new pmem device to the VM

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**pmemConfig** | [**PmemConfig**](PmemConfig.md)| The details of the new pmem device | 

### Return type

[**PciDeviceInfo**](PciDeviceInfo.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmAddVsockPut

> PciDeviceInfo VmAddVsockPut(ctx, vsockConfig)

Add a new vsock device to the VM

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**vsockConfig** | [**VsockConfig**](VsockConfig.md)| The details of the new vsock device | 

### Return type

[**PciDeviceInfo**](PciDeviceInfo.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmCountersGet

> map[string]map[string]int64 VmCountersGet(ctx, )

Get counters from the VM

### Required Parameters

This endpoint does not need any parameter.

### Return type

[**map[string]map[string]int64**](map.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmInfoGet

> VmInfo VmInfoGet(ctx, )

Returns general information about the cloud-hypervisor Virtual Machine (VM) instance.

### Required Parameters

This endpoint does not need any parameter.

### Return type

[**VmInfo**](VmInfo.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmRemoveDevicePut

> VmRemoveDevicePut(ctx, vmRemoveDevice)

Remove a device from the VM

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**vmRemoveDevice** | [**VmRemoveDevice**](VmRemoveDevice.md)| The identifier of the device | 

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmResizePut

> VmResizePut(ctx, vmResize)

Resize the VM

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**vmResize** | [**VmResize**](VmResize.md)| The target size for the VM | 

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmResizeZonePut

> VmResizeZonePut(ctx, vmResizeZone)

Resize a memory zone

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**vmResizeZone** | [**VmResizeZone**](VmResizeZone.md)| The target size for the memory zone | 

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmRestorePut

> VmRestorePut(ctx, restoreConfig)

Restore a VM from a snapshot.

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**restoreConfig** | [**RestoreConfig**](RestoreConfig.md)| The restore configuration | 

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmSnapshotPut

> VmSnapshotPut(ctx, vmSnapshotConfig)

Returns a VM snapshot.

### Required Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
**ctx** | **context.Context** | context for authentication, logging, cancellation, deadlines, tracing, etc.
**vmSnapshotConfig** | [**VmSnapshotConfig**](VmSnapshotConfig.md)| The snapshot configuration | 

### Return type

 (empty response body)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)


## VmmPingGet

> VmmPingResponse VmmPingGet(ctx, )

Ping the VMM to check for API server availability

### Required Parameters

This endpoint does not need any parameter.

### Return type

[**VmmPingResponse**](VmmPingResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints)
[[Back to Model list]](../README.md#documentation-for-models)
[[Back to README]](../README.md)

