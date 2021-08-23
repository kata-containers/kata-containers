# VmResizeZone

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | Pointer to **string** |  | [optional] 
**DesiredRam** | Pointer to **int64** | desired memory zone size in bytes | [optional] 

## Methods

### NewVmResizeZone

`func NewVmResizeZone() *VmResizeZone`

NewVmResizeZone instantiates a new VmResizeZone object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVmResizeZoneWithDefaults

`func NewVmResizeZoneWithDefaults() *VmResizeZone`

NewVmResizeZoneWithDefaults instantiates a new VmResizeZone object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *VmResizeZone) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *VmResizeZone) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *VmResizeZone) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *VmResizeZone) HasId() bool`

HasId returns a boolean if a field has been set.

### GetDesiredRam

`func (o *VmResizeZone) GetDesiredRam() int64`

GetDesiredRam returns the DesiredRam field if non-nil, zero value otherwise.

### GetDesiredRamOk

`func (o *VmResizeZone) GetDesiredRamOk() (*int64, bool)`

GetDesiredRamOk returns a tuple with the DesiredRam field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDesiredRam

`func (o *VmResizeZone) SetDesiredRam(v int64)`

SetDesiredRam sets DesiredRam field to given value.

### HasDesiredRam

`func (o *VmResizeZone) HasDesiredRam() bool`

HasDesiredRam returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


