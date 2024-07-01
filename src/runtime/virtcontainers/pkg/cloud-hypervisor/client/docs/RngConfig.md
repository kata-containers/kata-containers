# RngConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Src** | **string** |  | 
**Iommu** | Pointer to **bool** |  | [optional] [default to false]

## Methods

### NewRngConfig

`func NewRngConfig(src string, ) *RngConfig`

NewRngConfig instantiates a new RngConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewRngConfigWithDefaults

`func NewRngConfigWithDefaults() *RngConfig`

NewRngConfigWithDefaults instantiates a new RngConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSrc

`func (o *RngConfig) GetSrc() string`

GetSrc returns the Src field if non-nil, zero value otherwise.

### GetSrcOk

`func (o *RngConfig) GetSrcOk() (*string, bool)`

GetSrcOk returns a tuple with the Src field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSrc

`func (o *RngConfig) SetSrc(v string)`

SetSrc sets Src field to given value.


### GetIommu

`func (o *RngConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *RngConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *RngConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *RngConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


