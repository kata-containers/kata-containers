# VmResize

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**DesiredVcpus** | Pointer to **int32** |  | [optional] 
**DesiredRam** | Pointer to **int64** | desired memory ram in bytes | [optional] 
**DesiredBalloon** | Pointer to **int64** | desired balloon size in bytes | [optional] 

## Methods

### NewVmResize

`func NewVmResize() *VmResize`

NewVmResize instantiates a new VmResize object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVmResizeWithDefaults

`func NewVmResizeWithDefaults() *VmResize`

NewVmResizeWithDefaults instantiates a new VmResize object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetDesiredVcpus

`func (o *VmResize) GetDesiredVcpus() int32`

GetDesiredVcpus returns the DesiredVcpus field if non-nil, zero value otherwise.

### GetDesiredVcpusOk

`func (o *VmResize) GetDesiredVcpusOk() (*int32, bool)`

GetDesiredVcpusOk returns a tuple with the DesiredVcpus field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDesiredVcpus

`func (o *VmResize) SetDesiredVcpus(v int32)`

SetDesiredVcpus sets DesiredVcpus field to given value.

### HasDesiredVcpus

`func (o *VmResize) HasDesiredVcpus() bool`

HasDesiredVcpus returns a boolean if a field has been set.

### GetDesiredRam

`func (o *VmResize) GetDesiredRam() int64`

GetDesiredRam returns the DesiredRam field if non-nil, zero value otherwise.

### GetDesiredRamOk

`func (o *VmResize) GetDesiredRamOk() (*int64, bool)`

GetDesiredRamOk returns a tuple with the DesiredRam field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDesiredRam

`func (o *VmResize) SetDesiredRam(v int64)`

SetDesiredRam sets DesiredRam field to given value.

### HasDesiredRam

`func (o *VmResize) HasDesiredRam() bool`

HasDesiredRam returns a boolean if a field has been set.

### GetDesiredBalloon

`func (o *VmResize) GetDesiredBalloon() int64`

GetDesiredBalloon returns the DesiredBalloon field if non-nil, zero value otherwise.

### GetDesiredBalloonOk

`func (o *VmResize) GetDesiredBalloonOk() (*int64, bool)`

GetDesiredBalloonOk returns a tuple with the DesiredBalloon field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDesiredBalloon

`func (o *VmResize) SetDesiredBalloon(v int64)`

SetDesiredBalloon sets DesiredBalloon field to given value.

### HasDesiredBalloon

`func (o *VmResize) HasDesiredBalloon() bool`

HasDesiredBalloon returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


