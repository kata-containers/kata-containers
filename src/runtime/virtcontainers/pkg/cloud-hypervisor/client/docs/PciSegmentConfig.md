# PciSegmentConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**PciSegment** | **int32** |  | 
**Mmio32ApertureWeight** | Pointer to **int32** |  | [optional] 
**Mmio64ApertureWeight** | Pointer to **int32** |  | [optional] 

## Methods

### NewPciSegmentConfig

`func NewPciSegmentConfig(pciSegment int32, ) *PciSegmentConfig`

NewPciSegmentConfig instantiates a new PciSegmentConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewPciSegmentConfigWithDefaults

`func NewPciSegmentConfigWithDefaults() *PciSegmentConfig`

NewPciSegmentConfigWithDefaults instantiates a new PciSegmentConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetPciSegment

`func (o *PciSegmentConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *PciSegmentConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *PciSegmentConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.


### GetMmio32ApertureWeight

`func (o *PciSegmentConfig) GetMmio32ApertureWeight() int32`

GetMmio32ApertureWeight returns the Mmio32ApertureWeight field if non-nil, zero value otherwise.

### GetMmio32ApertureWeightOk

`func (o *PciSegmentConfig) GetMmio32ApertureWeightOk() (*int32, bool)`

GetMmio32ApertureWeightOk returns a tuple with the Mmio32ApertureWeight field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMmio32ApertureWeight

`func (o *PciSegmentConfig) SetMmio32ApertureWeight(v int32)`

SetMmio32ApertureWeight sets Mmio32ApertureWeight field to given value.

### HasMmio32ApertureWeight

`func (o *PciSegmentConfig) HasMmio32ApertureWeight() bool`

HasMmio32ApertureWeight returns a boolean if a field has been set.

### GetMmio64ApertureWeight

`func (o *PciSegmentConfig) GetMmio64ApertureWeight() int32`

GetMmio64ApertureWeight returns the Mmio64ApertureWeight field if non-nil, zero value otherwise.

### GetMmio64ApertureWeightOk

`func (o *PciSegmentConfig) GetMmio64ApertureWeightOk() (*int32, bool)`

GetMmio64ApertureWeightOk returns a tuple with the Mmio64ApertureWeight field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMmio64ApertureWeight

`func (o *PciSegmentConfig) SetMmio64ApertureWeight(v int32)`

SetMmio64ApertureWeight sets Mmio64ApertureWeight field to given value.

### HasMmio64ApertureWeight

`func (o *PciSegmentConfig) HasMmio64ApertureWeight() bool`

HasMmio64ApertureWeight returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


