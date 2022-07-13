# PmemConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**File** | **string** |  | 
**Size** | Pointer to **int64** |  | [optional] 
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**DiscardWrites** | Pointer to **bool** |  | [optional] [default to false]
**PciSegment** | Pointer to **int32** |  | [optional] 
**Id** | Pointer to **string** |  | [optional] 

## Methods

### NewPmemConfig

`func NewPmemConfig(file string, ) *PmemConfig`

NewPmemConfig instantiates a new PmemConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewPmemConfigWithDefaults

`func NewPmemConfigWithDefaults() *PmemConfig`

NewPmemConfigWithDefaults instantiates a new PmemConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetFile

`func (o *PmemConfig) GetFile() string`

GetFile returns the File field if non-nil, zero value otherwise.

### GetFileOk

`func (o *PmemConfig) GetFileOk() (*string, bool)`

GetFileOk returns a tuple with the File field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFile

`func (o *PmemConfig) SetFile(v string)`

SetFile sets File field to given value.


### GetSize

`func (o *PmemConfig) GetSize() int64`

GetSize returns the Size field if non-nil, zero value otherwise.

### GetSizeOk

`func (o *PmemConfig) GetSizeOk() (*int64, bool)`

GetSizeOk returns a tuple with the Size field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSize

`func (o *PmemConfig) SetSize(v int64)`

SetSize sets Size field to given value.

### HasSize

`func (o *PmemConfig) HasSize() bool`

HasSize returns a boolean if a field has been set.

### GetIommu

`func (o *PmemConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *PmemConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *PmemConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *PmemConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.

### GetDiscardWrites

`func (o *PmemConfig) GetDiscardWrites() bool`

GetDiscardWrites returns the DiscardWrites field if non-nil, zero value otherwise.

### GetDiscardWritesOk

`func (o *PmemConfig) GetDiscardWritesOk() (*bool, bool)`

GetDiscardWritesOk returns a tuple with the DiscardWrites field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDiscardWrites

`func (o *PmemConfig) SetDiscardWrites(v bool)`

SetDiscardWrites sets DiscardWrites field to given value.

### HasDiscardWrites

`func (o *PmemConfig) HasDiscardWrites() bool`

HasDiscardWrites returns a boolean if a field has been set.

### GetPciSegment

`func (o *PmemConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *PmemConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *PmemConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *PmemConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetId

`func (o *PmemConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *PmemConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *PmemConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *PmemConfig) HasId() bool`

HasId returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


