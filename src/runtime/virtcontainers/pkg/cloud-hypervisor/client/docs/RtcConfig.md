# RtcConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | Pointer to **string** |  | [optional]
**PciSegment** | Pointer to **int32** |  | [optional]
**PciDeviceId** | Pointer to **int32** |  | [optional]
**Iommu** | Pointer to **bool** |  | [optional] [default to false]

## Methods

### NewRtcConfig

`func NewRtcConfig() *RtcConfig`

NewRtcConfig instantiates a new RtcConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewRtcConfigWithDefaults

`func NewRtcConfigWithDefaults() *RtcConfig`

NewRtcConfigWithDefaults instantiates a new RtcConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *RtcConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *RtcConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *RtcConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *RtcConfig) HasId() bool`

HasId returns a boolean if a field has been set.

### GetPciSegment

`func (o *RtcConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *RtcConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *RtcConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *RtcConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetPciDeviceId

`func (o *RtcConfig) GetPciDeviceId() int32`

GetPciDeviceId returns the PciDeviceId field if non-nil, zero value otherwise.

### GetPciDeviceIdOk

`func (o *RtcConfig) GetPciDeviceIdOk() (*int32, bool)`

GetPciDeviceIdOk returns a tuple with the PciDeviceId field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciDeviceId

`func (o *RtcConfig) SetPciDeviceId(v int32)`

SetPciDeviceId sets PciDeviceId field to given value.

### HasPciDeviceId

`func (o *RtcConfig) HasPciDeviceId() bool`

HasPciDeviceId returns a boolean if a field has been set.

### GetIommu

`func (o *RtcConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *RtcConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *RtcConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *RtcConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
