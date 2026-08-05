# GenericVhostUserConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Socket** | **string** |  |
**QueueSizes** | **[]int32** |  |
**PciSegment** | Pointer to **int32** |  | [optional]
**PciDeviceId** | Pointer to **int32** |  | [optional]
**DeviceType** | **int32** |  |

## Methods

### NewGenericVhostUserConfig

`func NewGenericVhostUserConfig(socket string, queueSizes []int32, deviceType int32, ) *GenericVhostUserConfig`

NewGenericVhostUserConfig instantiates a new GenericVhostUserConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewGenericVhostUserConfigWithDefaults

`func NewGenericVhostUserConfigWithDefaults() *GenericVhostUserConfig`

NewGenericVhostUserConfigWithDefaults instantiates a new GenericVhostUserConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSocket

`func (o *GenericVhostUserConfig) GetSocket() string`

GetSocket returns the Socket field if non-nil, zero value otherwise.

### GetSocketOk

`func (o *GenericVhostUserConfig) GetSocketOk() (*string, bool)`

GetSocketOk returns a tuple with the Socket field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSocket

`func (o *GenericVhostUserConfig) SetSocket(v string)`

SetSocket sets Socket field to given value.


### GetQueueSizes

`func (o *GenericVhostUserConfig) GetQueueSizes() []int32`

GetQueueSizes returns the QueueSizes field if non-nil, zero value otherwise.

### GetQueueSizesOk

`func (o *GenericVhostUserConfig) GetQueueSizesOk() (*[]int32, bool)`

GetQueueSizesOk returns a tuple with the QueueSizes field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetQueueSizes

`func (o *GenericVhostUserConfig) SetQueueSizes(v []int32)`

SetQueueSizes sets QueueSizes field to given value.


### GetPciSegment

`func (o *GenericVhostUserConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *GenericVhostUserConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *GenericVhostUserConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *GenericVhostUserConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetPciDeviceId

`func (o *GenericVhostUserConfig) GetPciDeviceId() int32`

GetPciDeviceId returns the PciDeviceId field if non-nil, zero value otherwise.

### GetPciDeviceIdOk

`func (o *GenericVhostUserConfig) GetPciDeviceIdOk() (*int32, bool)`

GetPciDeviceIdOk returns a tuple with the PciDeviceId field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciDeviceId

`func (o *GenericVhostUserConfig) SetPciDeviceId(v int32)`

SetPciDeviceId sets PciDeviceId field to given value.

### HasPciDeviceId

`func (o *GenericVhostUserConfig) HasPciDeviceId() bool`

HasPciDeviceId returns a boolean if a field has been set.

### GetDeviceType

`func (o *GenericVhostUserConfig) GetDeviceType() int32`

GetDeviceType returns the DeviceType field if non-nil, zero value otherwise.

### GetDeviceTypeOk

`func (o *GenericVhostUserConfig) GetDeviceTypeOk() (*int32, bool)`

GetDeviceTypeOk returns a tuple with the DeviceType field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDeviceType

`func (o *GenericVhostUserConfig) SetDeviceType(v int32)`

SetDeviceType sets DeviceType field to given value.



[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
