# VmAddUserDevice

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Socket** | **string** |  |
**PciSegment** | Pointer to **int32** |  | [optional]
**PciDeviceId** | Pointer to **int32** |  | [optional]

## Methods

### NewVmAddUserDevice

`func NewVmAddUserDevice(socket string, ) *VmAddUserDevice`

NewVmAddUserDevice instantiates a new VmAddUserDevice object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVmAddUserDeviceWithDefaults

`func NewVmAddUserDeviceWithDefaults() *VmAddUserDevice`

NewVmAddUserDeviceWithDefaults instantiates a new VmAddUserDevice object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSocket

`func (o *VmAddUserDevice) GetSocket() string`

GetSocket returns the Socket field if non-nil, zero value otherwise.

### GetSocketOk

`func (o *VmAddUserDevice) GetSocketOk() (*string, bool)`

GetSocketOk returns a tuple with the Socket field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSocket

`func (o *VmAddUserDevice) SetSocket(v string)`

SetSocket sets Socket field to given value.


### GetPciSegment

`func (o *VmAddUserDevice) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *VmAddUserDevice) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *VmAddUserDevice) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *VmAddUserDevice) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetPciDeviceId

`func (o *VmAddUserDevice) GetPciDeviceId() int32`

GetPciDeviceId returns the PciDeviceId field if non-nil, zero value otherwise.

### GetPciDeviceIdOk

`func (o *VmAddUserDevice) GetPciDeviceIdOk() (*int32, bool)`

GetPciDeviceIdOk returns a tuple with the PciDeviceId field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciDeviceId

`func (o *VmAddUserDevice) SetPciDeviceId(v int32)`

SetPciDeviceId sets PciDeviceId field to given value.

### HasPciDeviceId

`func (o *VmAddUserDevice) HasPciDeviceId() bool`

HasPciDeviceId returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
