# VmInfo

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Config** | [**VmConfig**](VmConfig.md) |  | 
**State** | **string** |  | 
**MemoryActualSize** | Pointer to **int64** |  | [optional] 
**DeviceTree** | Pointer to [**map[string]DeviceNode**](DeviceNode.md) |  | [optional] 

## Methods

### NewVmInfo

`func NewVmInfo(config VmConfig, state string, ) *VmInfo`

NewVmInfo instantiates a new VmInfo object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVmInfoWithDefaults

`func NewVmInfoWithDefaults() *VmInfo`

NewVmInfoWithDefaults instantiates a new VmInfo object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetConfig

`func (o *VmInfo) GetConfig() VmConfig`

GetConfig returns the Config field if non-nil, zero value otherwise.

### GetConfigOk

`func (o *VmInfo) GetConfigOk() (*VmConfig, bool)`

GetConfigOk returns a tuple with the Config field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetConfig

`func (o *VmInfo) SetConfig(v VmConfig)`

SetConfig sets Config field to given value.


### GetState

`func (o *VmInfo) GetState() string`

GetState returns the State field if non-nil, zero value otherwise.

### GetStateOk

`func (o *VmInfo) GetStateOk() (*string, bool)`

GetStateOk returns a tuple with the State field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetState

`func (o *VmInfo) SetState(v string)`

SetState sets State field to given value.


### GetMemoryActualSize

`func (o *VmInfo) GetMemoryActualSize() int64`

GetMemoryActualSize returns the MemoryActualSize field if non-nil, zero value otherwise.

### GetMemoryActualSizeOk

`func (o *VmInfo) GetMemoryActualSizeOk() (*int64, bool)`

GetMemoryActualSizeOk returns a tuple with the MemoryActualSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMemoryActualSize

`func (o *VmInfo) SetMemoryActualSize(v int64)`

SetMemoryActualSize sets MemoryActualSize field to given value.

### HasMemoryActualSize

`func (o *VmInfo) HasMemoryActualSize() bool`

HasMemoryActualSize returns a boolean if a field has been set.

### GetDeviceTree

`func (o *VmInfo) GetDeviceTree() map[string]DeviceNode`

GetDeviceTree returns the DeviceTree field if non-nil, zero value otherwise.

### GetDeviceTreeOk

`func (o *VmInfo) GetDeviceTreeOk() (*map[string]DeviceNode, bool)`

GetDeviceTreeOk returns a tuple with the DeviceTree field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDeviceTree

`func (o *VmInfo) SetDeviceTree(v map[string]DeviceNode)`

SetDeviceTree sets DeviceTree field to given value.

### HasDeviceTree

`func (o *VmInfo) HasDeviceTree() bool`

HasDeviceTree returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


