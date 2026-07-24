# BalloonConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | Pointer to **string** |  | [optional]
**PciSegment** | Pointer to **int32** |  | [optional]
**PciDeviceId** | Pointer to **int32** |  | [optional]
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**Size** | **int64** |  |
**DeflateOnOom** | Pointer to **bool** | Deflate balloon when the guest is under memory pressure. | [optional] [default to false]
**FreePageReporting** | Pointer to **bool** | Enable guest to report free pages. | [optional] [default to false]

## Methods

### NewBalloonConfig

`func NewBalloonConfig(size int64, ) *BalloonConfig`

NewBalloonConfig instantiates a new BalloonConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewBalloonConfigWithDefaults

`func NewBalloonConfigWithDefaults() *BalloonConfig`

NewBalloonConfigWithDefaults instantiates a new BalloonConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *BalloonConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *BalloonConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *BalloonConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *BalloonConfig) HasId() bool`

HasId returns a boolean if a field has been set.

### GetPciSegment

`func (o *BalloonConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *BalloonConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *BalloonConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *BalloonConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetPciDeviceId

`func (o *BalloonConfig) GetPciDeviceId() int32`

GetPciDeviceId returns the PciDeviceId field if non-nil, zero value otherwise.

### GetPciDeviceIdOk

`func (o *BalloonConfig) GetPciDeviceIdOk() (*int32, bool)`

GetPciDeviceIdOk returns a tuple with the PciDeviceId field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciDeviceId

`func (o *BalloonConfig) SetPciDeviceId(v int32)`

SetPciDeviceId sets PciDeviceId field to given value.

### HasPciDeviceId

`func (o *BalloonConfig) HasPciDeviceId() bool`

HasPciDeviceId returns a boolean if a field has been set.

### GetIommu

`func (o *BalloonConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *BalloonConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *BalloonConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *BalloonConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.

### GetSize

`func (o *BalloonConfig) GetSize() int64`

GetSize returns the Size field if non-nil, zero value otherwise.

### GetSizeOk

`func (o *BalloonConfig) GetSizeOk() (*int64, bool)`

GetSizeOk returns a tuple with the Size field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSize

`func (o *BalloonConfig) SetSize(v int64)`

SetSize sets Size field to given value.


### GetDeflateOnOom

`func (o *BalloonConfig) GetDeflateOnOom() bool`

GetDeflateOnOom returns the DeflateOnOom field if non-nil, zero value otherwise.

### GetDeflateOnOomOk

`func (o *BalloonConfig) GetDeflateOnOomOk() (*bool, bool)`

GetDeflateOnOomOk returns a tuple with the DeflateOnOom field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDeflateOnOom

`func (o *BalloonConfig) SetDeflateOnOom(v bool)`

SetDeflateOnOom sets DeflateOnOom field to given value.

### HasDeflateOnOom

`func (o *BalloonConfig) HasDeflateOnOom() bool`

HasDeflateOnOom returns a boolean if a field has been set.

### GetFreePageReporting

`func (o *BalloonConfig) GetFreePageReporting() bool`

GetFreePageReporting returns the FreePageReporting field if non-nil, zero value otherwise.

### GetFreePageReportingOk

`func (o *BalloonConfig) GetFreePageReportingOk() (*bool, bool)`

GetFreePageReportingOk returns a tuple with the FreePageReporting field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFreePageReporting

`func (o *BalloonConfig) SetFreePageReporting(v bool)`

SetFreePageReporting sets FreePageReporting field to given value.

### HasFreePageReporting

`func (o *BalloonConfig) HasFreePageReporting() bool`

HasFreePageReporting returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
