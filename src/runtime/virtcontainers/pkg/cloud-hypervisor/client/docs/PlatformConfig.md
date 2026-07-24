# PlatformConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**NumPciSegments** | Pointer to **int32** |  | [optional]
**IommuSegments** | Pointer to **[]int32** |  | [optional]
**IommuAddressWidthBits** | Pointer to **int32** |  | [optional]
**SystemSerialNumber** | Pointer to **string** |  | [optional]
**SerialNumber** | Pointer to **string** |  | [optional]
**SystemUuid** | Pointer to **string** |  | [optional]
**Uuid** | Pointer to **string** |  | [optional]
**OemStrings** | Pointer to **[]string** |  | [optional]
**SystemManufacturer** | Pointer to **string** |  | [optional]
**SystemProductName** | Pointer to **string** |  | [optional]
**SystemVersion** | Pointer to **string** |  | [optional]
**SystemFamily** | Pointer to **string** |  | [optional]
**SystemSkuNumber** | Pointer to **string** |  | [optional]
**ChassisAssetTag** | Pointer to **string** |  | [optional]
**Tdx** | Pointer to **bool** |  | [optional] [default to false]
**SevSnp** | Pointer to **bool** |  | [optional] [default to false]
**Iommufd** | Pointer to **bool** |  | [optional] [default to false]
**VfioP2pDma** | Pointer to **bool** |  | [optional] [default to true]

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

### GetIommuAddressWidthBits

`func (o *PlatformConfig) GetIommuAddressWidthBits() int32`

GetIommuAddressWidthBits returns the IommuAddressWidthBits field if non-nil, zero value otherwise.

### GetIommuAddressWidthBitsOk

`func (o *PlatformConfig) GetIommuAddressWidthBitsOk() (*int32, bool)`

GetIommuAddressWidthBitsOk returns a tuple with the IommuAddressWidthBits field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommuAddressWidthBits

`func (o *PlatformConfig) SetIommuAddressWidthBits(v int32)`

SetIommuAddressWidthBits sets IommuAddressWidthBits field to given value.

### HasIommuAddressWidthBits

`func (o *PlatformConfig) HasIommuAddressWidthBits() bool`

HasIommuAddressWidthBits returns a boolean if a field has been set.

### GetSystemSerialNumber

`func (o *PlatformConfig) GetSystemSerialNumber() string`

GetSystemSerialNumber returns the SystemSerialNumber field if non-nil, zero value otherwise.

### GetSystemSerialNumberOk

`func (o *PlatformConfig) GetSystemSerialNumberOk() (*string, bool)`

GetSystemSerialNumberOk returns a tuple with the SystemSerialNumber field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSystemSerialNumber

`func (o *PlatformConfig) SetSystemSerialNumber(v string)`

SetSystemSerialNumber sets SystemSerialNumber field to given value.

### HasSystemSerialNumber

`func (o *PlatformConfig) HasSystemSerialNumber() bool`

HasSystemSerialNumber returns a boolean if a field has been set.

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

### GetSystemUuid

`func (o *PlatformConfig) GetSystemUuid() string`

GetSystemUuid returns the SystemUuid field if non-nil, zero value otherwise.

### GetSystemUuidOk

`func (o *PlatformConfig) GetSystemUuidOk() (*string, bool)`

GetSystemUuidOk returns a tuple with the SystemUuid field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSystemUuid

`func (o *PlatformConfig) SetSystemUuid(v string)`

SetSystemUuid sets SystemUuid field to given value.

### HasSystemUuid

`func (o *PlatformConfig) HasSystemUuid() bool`

HasSystemUuid returns a boolean if a field has been set.

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

### GetSystemManufacturer

`func (o *PlatformConfig) GetSystemManufacturer() string`

GetSystemManufacturer returns the SystemManufacturer field if non-nil, zero value otherwise.

### GetSystemManufacturerOk

`func (o *PlatformConfig) GetSystemManufacturerOk() (*string, bool)`

GetSystemManufacturerOk returns a tuple with the SystemManufacturer field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSystemManufacturer

`func (o *PlatformConfig) SetSystemManufacturer(v string)`

SetSystemManufacturer sets SystemManufacturer field to given value.

### HasSystemManufacturer

`func (o *PlatformConfig) HasSystemManufacturer() bool`

HasSystemManufacturer returns a boolean if a field has been set.

### GetSystemProductName

`func (o *PlatformConfig) GetSystemProductName() string`

GetSystemProductName returns the SystemProductName field if non-nil, zero value otherwise.

### GetSystemProductNameOk

`func (o *PlatformConfig) GetSystemProductNameOk() (*string, bool)`

GetSystemProductNameOk returns a tuple with the SystemProductName field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSystemProductName

`func (o *PlatformConfig) SetSystemProductName(v string)`

SetSystemProductName sets SystemProductName field to given value.

### HasSystemProductName

`func (o *PlatformConfig) HasSystemProductName() bool`

HasSystemProductName returns a boolean if a field has been set.

### GetSystemVersion

`func (o *PlatformConfig) GetSystemVersion() string`

GetSystemVersion returns the SystemVersion field if non-nil, zero value otherwise.

### GetSystemVersionOk

`func (o *PlatformConfig) GetSystemVersionOk() (*string, bool)`

GetSystemVersionOk returns a tuple with the SystemVersion field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSystemVersion

`func (o *PlatformConfig) SetSystemVersion(v string)`

SetSystemVersion sets SystemVersion field to given value.

### HasSystemVersion

`func (o *PlatformConfig) HasSystemVersion() bool`

HasSystemVersion returns a boolean if a field has been set.

### GetSystemFamily

`func (o *PlatformConfig) GetSystemFamily() string`

GetSystemFamily returns the SystemFamily field if non-nil, zero value otherwise.

### GetSystemFamilyOk

`func (o *PlatformConfig) GetSystemFamilyOk() (*string, bool)`

GetSystemFamilyOk returns a tuple with the SystemFamily field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSystemFamily

`func (o *PlatformConfig) SetSystemFamily(v string)`

SetSystemFamily sets SystemFamily field to given value.

### HasSystemFamily

`func (o *PlatformConfig) HasSystemFamily() bool`

HasSystemFamily returns a boolean if a field has been set.

### GetSystemSkuNumber

`func (o *PlatformConfig) GetSystemSkuNumber() string`

GetSystemSkuNumber returns the SystemSkuNumber field if non-nil, zero value otherwise.

### GetSystemSkuNumberOk

`func (o *PlatformConfig) GetSystemSkuNumberOk() (*string, bool)`

GetSystemSkuNumberOk returns a tuple with the SystemSkuNumber field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSystemSkuNumber

`func (o *PlatformConfig) SetSystemSkuNumber(v string)`

SetSystemSkuNumber sets SystemSkuNumber field to given value.

### HasSystemSkuNumber

`func (o *PlatformConfig) HasSystemSkuNumber() bool`

HasSystemSkuNumber returns a boolean if a field has been set.

### GetChassisAssetTag

`func (o *PlatformConfig) GetChassisAssetTag() string`

GetChassisAssetTag returns the ChassisAssetTag field if non-nil, zero value otherwise.

### GetChassisAssetTagOk

`func (o *PlatformConfig) GetChassisAssetTagOk() (*string, bool)`

GetChassisAssetTagOk returns a tuple with the ChassisAssetTag field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetChassisAssetTag

`func (o *PlatformConfig) SetChassisAssetTag(v string)`

SetChassisAssetTag sets ChassisAssetTag field to given value.

### HasChassisAssetTag

`func (o *PlatformConfig) HasChassisAssetTag() bool`

HasChassisAssetTag returns a boolean if a field has been set.

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

### GetSevSnp

`func (o *PlatformConfig) GetSevSnp() bool`

GetSevSnp returns the SevSnp field if non-nil, zero value otherwise.

### GetSevSnpOk

`func (o *PlatformConfig) GetSevSnpOk() (*bool, bool)`

GetSevSnpOk returns a tuple with the SevSnp field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSevSnp

`func (o *PlatformConfig) SetSevSnp(v bool)`

SetSevSnp sets SevSnp field to given value.

### HasSevSnp

`func (o *PlatformConfig) HasSevSnp() bool`

HasSevSnp returns a boolean if a field has been set.

### GetIommufd

`func (o *PlatformConfig) GetIommufd() bool`

GetIommufd returns the Iommufd field if non-nil, zero value otherwise.

### GetIommufdOk

`func (o *PlatformConfig) GetIommufdOk() (*bool, bool)`

GetIommufdOk returns a tuple with the Iommufd field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommufd

`func (o *PlatformConfig) SetIommufd(v bool)`

SetIommufd sets Iommufd field to given value.

### HasIommufd

`func (o *PlatformConfig) HasIommufd() bool`

HasIommufd returns a boolean if a field has been set.

### GetVfioP2pDma

`func (o *PlatformConfig) GetVfioP2pDma() bool`

GetVfioP2pDma returns the VfioP2pDma field if non-nil, zero value otherwise.

### GetVfioP2pDmaOk

`func (o *PlatformConfig) GetVfioP2pDmaOk() (*bool, bool)`

GetVfioP2pDmaOk returns a tuple with the VfioP2pDma field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVfioP2pDma

`func (o *PlatformConfig) SetVfioP2pDma(v bool)`

SetVfioP2pDma sets VfioP2pDma field to given value.

### HasVfioP2pDma

`func (o *PlatformConfig) HasVfioP2pDma() bool`

HasVfioP2pDma returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
