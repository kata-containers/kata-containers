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
[**VmAddUserDevicePut**](DefaultApi.md#VmAddUserDevicePut) | **Put** /vm.add-user-device | Add a new userspace device to the VM
[**VmAddVdpaPut**](DefaultApi.md#VmAddVdpaPut) | **Put** /vm.add-vdpa | Add a new vDPA device to the VM
[**VmAddVsockPut**](DefaultApi.md#VmAddVsockPut) | **Put** /vm.add-vsock | Add a new vsock device to the VM
[**VmCoredumpPut**](DefaultApi.md#VmCoredumpPut) | **Put** /vm.coredump | Takes a VM coredump.
[**VmCountersGet**](DefaultApi.md#VmCountersGet) | **Get** /vm.counters | Get counters from the VM
[**VmInfoGet**](DefaultApi.md#VmInfoGet) | **Get** /vm.info | Returns general information about the cloud-hypervisor Virtual Machine (VM) instance.
[**VmReceiveMigrationPut**](DefaultApi.md#VmReceiveMigrationPut) | **Put** /vm.receive-migration | Receive a VM migration from URL
[**VmRemoveDevicePut**](DefaultApi.md#VmRemoveDevicePut) | **Put** /vm.remove-device | Remove a device from the VM
[**VmResizePut**](DefaultApi.md#VmResizePut) | **Put** /vm.resize | Resize the VM
[**VmResizeZonePut**](DefaultApi.md#VmResizeZonePut) | **Put** /vm.resize-zone | Resize a memory zone
[**VmRestorePut**](DefaultApi.md#VmRestorePut) | **Put** /vm.restore | Restore a VM from a snapshot.
[**VmSendMigrationPut**](DefaultApi.md#VmSendMigrationPut) | **Put** /vm.send-migration | Send a VM migration to URL
[**VmSnapshotPut**](DefaultApi.md#VmSnapshotPut) | **Put** /vm.snapshot | Returns a VM snapshot.
[**VmmNmiPut**](DefaultApi.md#VmmNmiPut) | **Put** /vmm.nmi | Inject an NMI.
[**VmmPingGet**](DefaultApi.md#VmmPingGet) | **Get** /vmm.ping | Ping the VMM to check for API server availability



## BootVM

> BootVM(ctx).Execute()

Boot the previously created VM instance.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.BootVM(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.BootVM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiBootVMRequest struct via the builder pattern


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

> CreateVM(ctx).VmConfig(vmConfig).Execute()

Create the cloud-hypervisor Virtual Machine (VM) instance. The instance is not booted, only created.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vmConfig := *openapiclient.NewVmConfig(*openapiclient.NewPayloadConfig()) // VmConfig | The VM configuration

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.CreateVM(context.Background()).VmConfig(vmConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.CreateVM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiCreateVMRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vmConfig** | [**VmConfig**](VmConfig.md) | The VM configuration | 

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

> DeleteVM(ctx).Execute()

Delete the cloud-hypervisor Virtual Machine (VM) instance.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.DeleteVM(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.DeleteVM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiDeleteVMRequest struct via the builder pattern


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

> PauseVM(ctx).Execute()

Pause a previously booted VM instance.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.PauseVM(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.PauseVM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiPauseVMRequest struct via the builder pattern


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

> PowerButtonVM(ctx).Execute()

Trigger a power button in the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.PowerButtonVM(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.PowerButtonVM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiPowerButtonVMRequest struct via the builder pattern


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

> RebootVM(ctx).Execute()

Reboot the VM instance.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.RebootVM(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.RebootVM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiRebootVMRequest struct via the builder pattern


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

> ResumeVM(ctx).Execute()

Resume a previously paused VM instance.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.ResumeVM(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.ResumeVM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiResumeVMRequest struct via the builder pattern


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

> ShutdownVM(ctx).Execute()

Shut the VM instance down.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.ShutdownVM(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.ShutdownVM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiShutdownVMRequest struct via the builder pattern


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

> ShutdownVMM(ctx).Execute()

Shuts the cloud-hypervisor VMM.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.ShutdownVMM(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.ShutdownVMM``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiShutdownVMMRequest struct via the builder pattern


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

> PciDeviceInfo VmAddDevicePut(ctx).DeviceConfig(deviceConfig).Execute()

Add a new device to the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    deviceConfig := *openapiclient.NewDeviceConfig("Path_example") // DeviceConfig | The path of the new device

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmAddDevicePut(context.Background()).DeviceConfig(deviceConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmAddDevicePut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmAddDevicePut`: PciDeviceInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmAddDevicePut`: %v\n", resp)
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmAddDevicePutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **deviceConfig** | [**DeviceConfig**](DeviceConfig.md) | The path of the new device | 

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

> PciDeviceInfo VmAddDiskPut(ctx).DiskConfig(diskConfig).Execute()

Add a new disk to the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    diskConfig := *openapiclient.NewDiskConfig("Path_example") // DiskConfig | The details of the new disk

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmAddDiskPut(context.Background()).DiskConfig(diskConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmAddDiskPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmAddDiskPut`: PciDeviceInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmAddDiskPut`: %v\n", resp)
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmAddDiskPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **diskConfig** | [**DiskConfig**](DiskConfig.md) | The details of the new disk | 

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

> PciDeviceInfo VmAddFsPut(ctx).FsConfig(fsConfig).Execute()

Add a new virtio-fs device to the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    fsConfig := *openapiclient.NewFsConfig("Tag_example", "Socket_example", int32(123), int32(123)) // FsConfig | The details of the new virtio-fs

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmAddFsPut(context.Background()).FsConfig(fsConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmAddFsPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmAddFsPut`: PciDeviceInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmAddFsPut`: %v\n", resp)
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmAddFsPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **fsConfig** | [**FsConfig**](FsConfig.md) | The details of the new virtio-fs | 

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

> PciDeviceInfo VmAddNetPut(ctx).NetConfig(netConfig).Execute()

Add a new network device to the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    netConfig := *openapiclient.NewNetConfig() // NetConfig | The details of the new network device

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmAddNetPut(context.Background()).NetConfig(netConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmAddNetPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmAddNetPut`: PciDeviceInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmAddNetPut`: %v\n", resp)
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmAddNetPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **netConfig** | [**NetConfig**](NetConfig.md) | The details of the new network device | 

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

> PciDeviceInfo VmAddPmemPut(ctx).PmemConfig(pmemConfig).Execute()

Add a new pmem device to the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    pmemConfig := *openapiclient.NewPmemConfig("File_example") // PmemConfig | The details of the new pmem device

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmAddPmemPut(context.Background()).PmemConfig(pmemConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmAddPmemPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmAddPmemPut`: PciDeviceInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmAddPmemPut`: %v\n", resp)
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmAddPmemPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **pmemConfig** | [**PmemConfig**](PmemConfig.md) | The details of the new pmem device | 

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


## VmAddUserDevicePut

> PciDeviceInfo VmAddUserDevicePut(ctx).VmAddUserDevice(vmAddUserDevice).Execute()

Add a new userspace device to the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vmAddUserDevice := *openapiclient.NewVmAddUserDevice("Socket_example") // VmAddUserDevice | The path of the new device

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmAddUserDevicePut(context.Background()).VmAddUserDevice(vmAddUserDevice).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmAddUserDevicePut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmAddUserDevicePut`: PciDeviceInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmAddUserDevicePut`: %v\n", resp)
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmAddUserDevicePutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vmAddUserDevice** | [**VmAddUserDevice**](VmAddUserDevice.md) | The path of the new device | 

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


## VmAddVdpaPut

> PciDeviceInfo VmAddVdpaPut(ctx).VdpaConfig(vdpaConfig).Execute()

Add a new vDPA device to the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vdpaConfig := *openapiclient.NewVdpaConfig("Path_example", int32(123)) // VdpaConfig | The details of the new vDPA device

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmAddVdpaPut(context.Background()).VdpaConfig(vdpaConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmAddVdpaPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmAddVdpaPut`: PciDeviceInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmAddVdpaPut`: %v\n", resp)
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmAddVdpaPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vdpaConfig** | [**VdpaConfig**](VdpaConfig.md) | The details of the new vDPA device | 

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

> PciDeviceInfo VmAddVsockPut(ctx).VsockConfig(vsockConfig).Execute()

Add a new vsock device to the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vsockConfig := *openapiclient.NewVsockConfig(int64(123), "Socket_example") // VsockConfig | The details of the new vsock device

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmAddVsockPut(context.Background()).VsockConfig(vsockConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmAddVsockPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmAddVsockPut`: PciDeviceInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmAddVsockPut`: %v\n", resp)
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmAddVsockPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vsockConfig** | [**VsockConfig**](VsockConfig.md) | The details of the new vsock device | 

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


## VmCoredumpPut

> VmCoredumpPut(ctx).VmCoredumpData(vmCoredumpData).Execute()

Takes a VM coredump.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vmCoredumpData := *openapiclient.NewVmCoredumpData() // VmCoredumpData | The coredump configuration

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmCoredumpPut(context.Background()).VmCoredumpData(vmCoredumpData).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmCoredumpPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmCoredumpPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vmCoredumpData** | [**VmCoredumpData**](VmCoredumpData.md) | The coredump configuration | 

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


## VmCountersGet

> map[string]map[string]int64 VmCountersGet(ctx).Execute()

Get counters from the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmCountersGet(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmCountersGet``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmCountersGet`: map[string]map[string]int64
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmCountersGet`: %v\n", resp)
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiVmCountersGetRequest struct via the builder pattern


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

> VmInfo VmInfoGet(ctx).Execute()

Returns general information about the cloud-hypervisor Virtual Machine (VM) instance.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmInfoGet(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmInfoGet``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmInfoGet`: VmInfo
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmInfoGet`: %v\n", resp)
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiVmInfoGetRequest struct via the builder pattern


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


## VmReceiveMigrationPut

> VmReceiveMigrationPut(ctx).ReceiveMigrationData(receiveMigrationData).Execute()

Receive a VM migration from URL

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    receiveMigrationData := *openapiclient.NewReceiveMigrationData("ReceiverUrl_example") // ReceiveMigrationData | The URL for the reception of migration state

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmReceiveMigrationPut(context.Background()).ReceiveMigrationData(receiveMigrationData).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmReceiveMigrationPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmReceiveMigrationPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **receiveMigrationData** | [**ReceiveMigrationData**](ReceiveMigrationData.md) | The URL for the reception of migration state | 

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


## VmRemoveDevicePut

> VmRemoveDevicePut(ctx).VmRemoveDevice(vmRemoveDevice).Execute()

Remove a device from the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vmRemoveDevice := *openapiclient.NewVmRemoveDevice() // VmRemoveDevice | The identifier of the device

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmRemoveDevicePut(context.Background()).VmRemoveDevice(vmRemoveDevice).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmRemoveDevicePut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmRemoveDevicePutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vmRemoveDevice** | [**VmRemoveDevice**](VmRemoveDevice.md) | The identifier of the device | 

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

> VmResizePut(ctx).VmResize(vmResize).Execute()

Resize the VM

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vmResize := *openapiclient.NewVmResize() // VmResize | The target size for the VM

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmResizePut(context.Background()).VmResize(vmResize).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmResizePut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmResizePutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vmResize** | [**VmResize**](VmResize.md) | The target size for the VM | 

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

> VmResizeZonePut(ctx).VmResizeZone(vmResizeZone).Execute()

Resize a memory zone

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vmResizeZone := *openapiclient.NewVmResizeZone() // VmResizeZone | The target size for the memory zone

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmResizeZonePut(context.Background()).VmResizeZone(vmResizeZone).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmResizeZonePut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmResizeZonePutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vmResizeZone** | [**VmResizeZone**](VmResizeZone.md) | The target size for the memory zone | 

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

> VmRestorePut(ctx).RestoreConfig(restoreConfig).Execute()

Restore a VM from a snapshot.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    restoreConfig := *openapiclient.NewRestoreConfig("SourceUrl_example") // RestoreConfig | The restore configuration

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmRestorePut(context.Background()).RestoreConfig(restoreConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmRestorePut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmRestorePutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **restoreConfig** | [**RestoreConfig**](RestoreConfig.md) | The restore configuration | 

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


## VmSendMigrationPut

> VmSendMigrationPut(ctx).SendMigrationData(sendMigrationData).Execute()

Send a VM migration to URL

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    sendMigrationData := *openapiclient.NewSendMigrationData("DestinationUrl_example") // SendMigrationData | The URL for sending the migration state

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmSendMigrationPut(context.Background()).SendMigrationData(sendMigrationData).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmSendMigrationPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmSendMigrationPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **sendMigrationData** | [**SendMigrationData**](SendMigrationData.md) | The URL for sending the migration state | 

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

> VmSnapshotPut(ctx).VmSnapshotConfig(vmSnapshotConfig).Execute()

Returns a VM snapshot.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {
    vmSnapshotConfig := *openapiclient.NewVmSnapshotConfig() // VmSnapshotConfig | The snapshot configuration

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmSnapshotPut(context.Background()).VmSnapshotConfig(vmSnapshotConfig).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmSnapshotPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters



### Other Parameters

Other parameters are passed through a pointer to a apiVmSnapshotPutRequest struct via the builder pattern


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **vmSnapshotConfig** | [**VmSnapshotConfig**](VmSnapshotConfig.md) | The snapshot configuration | 

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


## VmmNmiPut

> VmmNmiPut(ctx).Execute()

Inject an NMI.

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmmNmiPut(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmmNmiPut``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiVmmNmiPutRequest struct via the builder pattern


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


## VmmPingGet

> VmmPingResponse VmmPingGet(ctx).Execute()

Ping the VMM to check for API server availability

### Example

```go
package main

import (
    "context"
    "fmt"
    "os"
    openapiclient "./openapi"
)

func main() {

    configuration := openapiclient.NewConfiguration()
    api_client := openapiclient.NewAPIClient(configuration)
    resp, r, err := api_client.DefaultApi.VmmPingGet(context.Background()).Execute()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Error when calling `DefaultApi.VmmPingGet``: %v\n", err)
        fmt.Fprintf(os.Stderr, "Full HTTP response: %v\n", r)
    }
    // response from `VmmPingGet`: VmmPingResponse
    fmt.Fprintf(os.Stdout, "Response from `DefaultApi.VmmPingGet`: %v\n", resp)
}
```

### Path Parameters

This endpoint does not need any parameter.

### Other Parameters

Other parameters are passed through a pointer to a apiVmmPingGetRequest struct via the builder pattern


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

