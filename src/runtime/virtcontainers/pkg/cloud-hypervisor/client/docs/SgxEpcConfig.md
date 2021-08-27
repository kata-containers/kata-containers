# SgxEpcConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | **string** |  | 
**Size** | **int64** |  | 
**Prefault** | Pointer to **bool** |  | [optional] [default to false]

## Methods

### NewSgxEpcConfig

`func NewSgxEpcConfig(id string, size int64, ) *SgxEpcConfig`

NewSgxEpcConfig instantiates a new SgxEpcConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewSgxEpcConfigWithDefaults

`func NewSgxEpcConfigWithDefaults() *SgxEpcConfig`

NewSgxEpcConfigWithDefaults instantiates a new SgxEpcConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *SgxEpcConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *SgxEpcConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *SgxEpcConfig) SetId(v string)`

SetId sets Id field to given value.


### GetSize

`func (o *SgxEpcConfig) GetSize() int64`

GetSize returns the Size field if non-nil, zero value otherwise.

### GetSizeOk

`func (o *SgxEpcConfig) GetSizeOk() (*int64, bool)`

GetSizeOk returns a tuple with the Size field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSize

`func (o *SgxEpcConfig) SetSize(v int64)`

SetSize sets Size field to given value.


### GetPrefault

`func (o *SgxEpcConfig) GetPrefault() bool`

GetPrefault returns the Prefault field if non-nil, zero value otherwise.

### GetPrefaultOk

`func (o *SgxEpcConfig) GetPrefaultOk() (*bool, bool)`

GetPrefaultOk returns a tuple with the Prefault field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPrefault

`func (o *SgxEpcConfig) SetPrefault(v bool)`

SetPrefault sets Prefault field to given value.

### HasPrefault

`func (o *SgxEpcConfig) HasPrefault() bool`

HasPrefault returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


