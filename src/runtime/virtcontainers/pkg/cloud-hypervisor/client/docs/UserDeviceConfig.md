# UserDeviceConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Socket** | **string** |  |
**Id** | Pointer to **string** |  | [optional]
**PciSegment** | Pointer to **int32** |  | [optional]
**PciDeviceId** | Pointer to **int32** |  | [optional]

## Methods

### NewUserDeviceConfig

`func NewUserDeviceConfig(socket string, ) *UserDeviceConfig`

NewUserDeviceConfig instantiates a new UserDeviceConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewUserDeviceConfigWithDefaults

`func NewUserDeviceConfigWithDefaults() *UserDeviceConfig`

NewUserDeviceConfigWithDefaults instantiates a new UserDeviceConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSocket

`func (o *UserDeviceConfig) GetSocket() string`

GetSocket returns the Socket field if non-nil, zero value otherwise.

### GetSocketOk

`func (o *UserDeviceConfig) GetSocketOk() (*string, bool)`

GetSocketOk returns a tuple with the Socket field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSocket

`func (o *UserDeviceConfig) SetSocket(v string)`

SetSocket sets Socket field to given value.


### GetId

`func (o *UserDeviceConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *UserDeviceConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *UserDeviceConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *UserDeviceConfig) HasId() bool`

HasId returns a boolean if a field has been set.

### GetPciSegment

`func (o *UserDeviceConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *UserDeviceConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *UserDeviceConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *UserDeviceConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetPciDeviceId

`func (o *UserDeviceConfig) GetPciDeviceId() int32`

GetPciDeviceId returns the PciDeviceId field if non-nil, zero value otherwise.

### GetPciDeviceIdOk

`func (o *UserDeviceConfig) GetPciDeviceIdOk() (*int32, bool)`

GetPciDeviceIdOk returns a tuple with the PciDeviceId field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciDeviceId

`func (o *UserDeviceConfig) SetPciDeviceId(v int32)`

SetPciDeviceId sets PciDeviceId field to given value.

### HasPciDeviceId

`func (o *UserDeviceConfig) HasPciDeviceId() bool`

HasPciDeviceId returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
