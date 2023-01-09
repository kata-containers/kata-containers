# MemoryConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Size** | **int64** |  | 
**HotplugSize** | Pointer to **int64** |  | [optional] 
**HotpluggedSize** | Pointer to **int64** |  | [optional] 
**Mergeable** | Pointer to **bool** |  | [optional] [default to false]
**HotplugMethod** | Pointer to **string** |  | [optional] [default to "Acpi"]
**Shared** | Pointer to **bool** |  | [optional] [default to false]
**Hugepages** | Pointer to **bool** |  | [optional] [default to false]
**HugepageSize** | Pointer to **int64** |  | [optional] 
**Prefault** | Pointer to **bool** |  | [optional] [default to false]
**Thp** | Pointer to **bool** |  | [optional] [default to true]
**Zones** | Pointer to [**[]MemoryZoneConfig**](MemoryZoneConfig.md) |  | [optional] 

## Methods

### NewMemoryConfig

`func NewMemoryConfig(size int64, ) *MemoryConfig`

NewMemoryConfig instantiates a new MemoryConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewMemoryConfigWithDefaults

`func NewMemoryConfigWithDefaults() *MemoryConfig`

NewMemoryConfigWithDefaults instantiates a new MemoryConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSize

`func (o *MemoryConfig) GetSize() int64`

GetSize returns the Size field if non-nil, zero value otherwise.

### GetSizeOk

`func (o *MemoryConfig) GetSizeOk() (*int64, bool)`

GetSizeOk returns a tuple with the Size field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSize

`func (o *MemoryConfig) SetSize(v int64)`

SetSize sets Size field to given value.


### GetHotplugSize

`func (o *MemoryConfig) GetHotplugSize() int64`

GetHotplugSize returns the HotplugSize field if non-nil, zero value otherwise.

### GetHotplugSizeOk

`func (o *MemoryConfig) GetHotplugSizeOk() (*int64, bool)`

GetHotplugSizeOk returns a tuple with the HotplugSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHotplugSize

`func (o *MemoryConfig) SetHotplugSize(v int64)`

SetHotplugSize sets HotplugSize field to given value.

### HasHotplugSize

`func (o *MemoryConfig) HasHotplugSize() bool`

HasHotplugSize returns a boolean if a field has been set.

### GetHotpluggedSize

`func (o *MemoryConfig) GetHotpluggedSize() int64`

GetHotpluggedSize returns the HotpluggedSize field if non-nil, zero value otherwise.

### GetHotpluggedSizeOk

`func (o *MemoryConfig) GetHotpluggedSizeOk() (*int64, bool)`

GetHotpluggedSizeOk returns a tuple with the HotpluggedSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHotpluggedSize

`func (o *MemoryConfig) SetHotpluggedSize(v int64)`

SetHotpluggedSize sets HotpluggedSize field to given value.

### HasHotpluggedSize

`func (o *MemoryConfig) HasHotpluggedSize() bool`

HasHotpluggedSize returns a boolean if a field has been set.

### GetMergeable

`func (o *MemoryConfig) GetMergeable() bool`

GetMergeable returns the Mergeable field if non-nil, zero value otherwise.

### GetMergeableOk

`func (o *MemoryConfig) GetMergeableOk() (*bool, bool)`

GetMergeableOk returns a tuple with the Mergeable field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMergeable

`func (o *MemoryConfig) SetMergeable(v bool)`

SetMergeable sets Mergeable field to given value.

### HasMergeable

`func (o *MemoryConfig) HasMergeable() bool`

HasMergeable returns a boolean if a field has been set.

### GetHotplugMethod

`func (o *MemoryConfig) GetHotplugMethod() string`

GetHotplugMethod returns the HotplugMethod field if non-nil, zero value otherwise.

### GetHotplugMethodOk

`func (o *MemoryConfig) GetHotplugMethodOk() (*string, bool)`

GetHotplugMethodOk returns a tuple with the HotplugMethod field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHotplugMethod

`func (o *MemoryConfig) SetHotplugMethod(v string)`

SetHotplugMethod sets HotplugMethod field to given value.

### HasHotplugMethod

`func (o *MemoryConfig) HasHotplugMethod() bool`

HasHotplugMethod returns a boolean if a field has been set.

### GetShared

`func (o *MemoryConfig) GetShared() bool`

GetShared returns the Shared field if non-nil, zero value otherwise.

### GetSharedOk

`func (o *MemoryConfig) GetSharedOk() (*bool, bool)`

GetSharedOk returns a tuple with the Shared field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetShared

`func (o *MemoryConfig) SetShared(v bool)`

SetShared sets Shared field to given value.

### HasShared

`func (o *MemoryConfig) HasShared() bool`

HasShared returns a boolean if a field has been set.

### GetHugepages

`func (o *MemoryConfig) GetHugepages() bool`

GetHugepages returns the Hugepages field if non-nil, zero value otherwise.

### GetHugepagesOk

`func (o *MemoryConfig) GetHugepagesOk() (*bool, bool)`

GetHugepagesOk returns a tuple with the Hugepages field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHugepages

`func (o *MemoryConfig) SetHugepages(v bool)`

SetHugepages sets Hugepages field to given value.

### HasHugepages

`func (o *MemoryConfig) HasHugepages() bool`

HasHugepages returns a boolean if a field has been set.

### GetHugepageSize

`func (o *MemoryConfig) GetHugepageSize() int64`

GetHugepageSize returns the HugepageSize field if non-nil, zero value otherwise.

### GetHugepageSizeOk

`func (o *MemoryConfig) GetHugepageSizeOk() (*int64, bool)`

GetHugepageSizeOk returns a tuple with the HugepageSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHugepageSize

`func (o *MemoryConfig) SetHugepageSize(v int64)`

SetHugepageSize sets HugepageSize field to given value.

### HasHugepageSize

`func (o *MemoryConfig) HasHugepageSize() bool`

HasHugepageSize returns a boolean if a field has been set.

### GetPrefault

`func (o *MemoryConfig) GetPrefault() bool`

GetPrefault returns the Prefault field if non-nil, zero value otherwise.

### GetPrefaultOk

`func (o *MemoryConfig) GetPrefaultOk() (*bool, bool)`

GetPrefaultOk returns a tuple with the Prefault field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPrefault

`func (o *MemoryConfig) SetPrefault(v bool)`

SetPrefault sets Prefault field to given value.

### HasPrefault

`func (o *MemoryConfig) HasPrefault() bool`

HasPrefault returns a boolean if a field has been set.

### GetThp

`func (o *MemoryConfig) GetThp() bool`

GetThp returns the Thp field if non-nil, zero value otherwise.

### GetThpOk

`func (o *MemoryConfig) GetThpOk() (*bool, bool)`

GetThpOk returns a tuple with the Thp field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetThp

`func (o *MemoryConfig) SetThp(v bool)`

SetThp sets Thp field to given value.

### HasThp

`func (o *MemoryConfig) HasThp() bool`

HasThp returns a boolean if a field has been set.

### GetZones

`func (o *MemoryConfig) GetZones() []MemoryZoneConfig`

GetZones returns the Zones field if non-nil, zero value otherwise.

### GetZonesOk

`func (o *MemoryConfig) GetZonesOk() (*[]MemoryZoneConfig, bool)`

GetZonesOk returns a tuple with the Zones field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetZones

`func (o *MemoryConfig) SetZones(v []MemoryZoneConfig)`

SetZones sets Zones field to given value.

### HasZones

`func (o *MemoryConfig) HasZones() bool`

HasZones returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


