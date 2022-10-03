# PlatformConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**NumPciSegments** | Pointer to **int32** |  | [optional] 
**IommuSegments** | Pointer to **[]int32** |  | [optional] 
**SerialNumber** | Pointer to **string** |  | [optional] 
**Uuid** | Pointer to **string** |  | [optional] 
**OemStrings** | Pointer to **[]string** |  | [optional] 
**Tdx** | Pointer to **bool** |  | [optional] [default to false]

## Methods

### NewPlatformConfig

`func NewPlatformConfig() *PlatformConfig`

NewPlatformConfig instantiates a new PlatformConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewPlatformConfigWithDefaults

`func NewPlatformConfigWithDefaults() *PlatformConfig`

NewPlatformConfigWithDefaults instantiates a new PlatformConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetNumPciSegments

`func (o *PlatformConfig) GetNumPciSegments() int32`

GetNumPciSegments returns the NumPciSegments field if non-nil, zero value otherwise.

### GetNumPciSegmentsOk

`func (o *PlatformConfig) GetNumPciSegmentsOk() (*int32, bool)`

GetNumPciSegmentsOk returns a tuple with the NumPciSegments field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetNumPciSegments

`func (o *PlatformConfig) SetNumPciSegments(v int32)`

SetNumPciSegments sets NumPciSegments field to given value.

### HasNumPciSegments

`func (o *PlatformConfig) HasNumPciSegments() bool`

HasNumPciSegments returns a boolean if a field has been set.

### GetIommuSegments

`func (o *PlatformConfig) GetIommuSegments() []int32`

GetIommuSegments returns the IommuSegments field if non-nil, zero value otherwise.

### GetIommuSegmentsOk

`func (o *PlatformConfig) GetIommuSegmentsOk() (*[]int32, bool)`

GetIommuSegmentsOk returns a tuple with the IommuSegments field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommuSegments

`func (o *PlatformConfig) SetIommuSegments(v []int32)`

SetIommuSegments sets IommuSegments field to given value.

### HasIommuSegments

`func (o *PlatformConfig) HasIommuSegments() bool`

HasIommuSegments returns a boolean if a field has been set.

### GetSerialNumber

`func (o *PlatformConfig) GetSerialNumber() string`

GetSerialNumber returns the SerialNumber field if non-nil, zero value otherwise.

### GetSerialNumberOk

`func (o *PlatformConfig) GetSerialNumberOk() (*string, bool)`

GetSerialNumberOk returns a tuple with the SerialNumber field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSerialNumber

`func (o *PlatformConfig) SetSerialNumber(v string)`

SetSerialNumber sets SerialNumber field to given value.

### HasSerialNumber

`func (o *PlatformConfig) HasSerialNumber() bool`

HasSerialNumber returns a boolean if a field has been set.

### GetUuid

`func (o *PlatformConfig) GetUuid() string`

GetUuid returns the Uuid field if non-nil, zero value otherwise.

### GetUuidOk

`func (o *PlatformConfig) GetUuidOk() (*string, bool)`

GetUuidOk returns a tuple with the Uuid field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetUuid

`func (o *PlatformConfig) SetUuid(v string)`

SetUuid sets Uuid field to given value.

### HasUuid

`func (o *PlatformConfig) HasUuid() bool`

HasUuid returns a boolean if a field has been set.

### GetOemStrings

`func (o *PlatformConfig) GetOemStrings() []string`

GetOemStrings returns the OemStrings field if non-nil, zero value otherwise.

### GetOemStringsOk

`func (o *PlatformConfig) GetOemStringsOk() (*[]string, bool)`

GetOemStringsOk returns a tuple with the OemStrings field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetOemStrings

`func (o *PlatformConfig) SetOemStrings(v []string)`

SetOemStrings sets OemStrings field to given value.

### HasOemStrings

`func (o *PlatformConfig) HasOemStrings() bool`

HasOemStrings returns a boolean if a field has been set.

### GetTdx

`func (o *PlatformConfig) GetTdx() bool`

GetTdx returns the Tdx field if non-nil, zero value otherwise.

### GetTdxOk

`func (o *PlatformConfig) GetTdxOk() (*bool, bool)`

GetTdxOk returns a tuple with the Tdx field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTdx

`func (o *PlatformConfig) SetTdx(v bool)`

SetTdx sets Tdx field to given value.

### HasTdx

`func (o *PlatformConfig) HasTdx() bool`

HasTdx returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


