# MemoryZoneConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | **string** |  | 
**Size** | **int64** |  | 
**File** | Pointer to **string** |  | [optional] 
**Mergeable** | Pointer to **bool** |  | [optional] [default to false]
**Shared** | Pointer to **bool** |  | [optional] [default to false]
**Hugepages** | Pointer to **bool** |  | [optional] [default to false]
**HugepageSize** | Pointer to **int64** |  | [optional] 
**HostNumaNode** | Pointer to **int32** |  | [optional] 
**HotplugSize** | Pointer to **int64** |  | [optional] 
**HotpluggedSize** | Pointer to **int64** |  | [optional] 
**Prefault** | Pointer to **bool** |  | [optional] [default to false]

## Methods

### NewMemoryZoneConfig

`func NewMemoryZoneConfig(id string, size int64, ) *MemoryZoneConfig`

NewMemoryZoneConfig instantiates a new MemoryZoneConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewMemoryZoneConfigWithDefaults

`func NewMemoryZoneConfigWithDefaults() *MemoryZoneConfig`

NewMemoryZoneConfigWithDefaults instantiates a new MemoryZoneConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *MemoryZoneConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *MemoryZoneConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *MemoryZoneConfig) SetId(v string)`

SetId sets Id field to given value.


### GetSize

`func (o *MemoryZoneConfig) GetSize() int64`

GetSize returns the Size field if non-nil, zero value otherwise.

### GetSizeOk

`func (o *MemoryZoneConfig) GetSizeOk() (*int64, bool)`

GetSizeOk returns a tuple with the Size field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSize

`func (o *MemoryZoneConfig) SetSize(v int64)`

SetSize sets Size field to given value.


### GetFile

`func (o *MemoryZoneConfig) GetFile() string`

GetFile returns the File field if non-nil, zero value otherwise.

### GetFileOk

`func (o *MemoryZoneConfig) GetFileOk() (*string, bool)`

GetFileOk returns a tuple with the File field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFile

`func (o *MemoryZoneConfig) SetFile(v string)`

SetFile sets File field to given value.

### HasFile

`func (o *MemoryZoneConfig) HasFile() bool`

HasFile returns a boolean if a field has been set.

### GetMergeable

`func (o *MemoryZoneConfig) GetMergeable() bool`

GetMergeable returns the Mergeable field if non-nil, zero value otherwise.

### GetMergeableOk

`func (o *MemoryZoneConfig) GetMergeableOk() (*bool, bool)`

GetMergeableOk returns a tuple with the Mergeable field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMergeable

`func (o *MemoryZoneConfig) SetMergeable(v bool)`

SetMergeable sets Mergeable field to given value.

### HasMergeable

`func (o *MemoryZoneConfig) HasMergeable() bool`

HasMergeable returns a boolean if a field has been set.

### GetShared

`func (o *MemoryZoneConfig) GetShared() bool`

GetShared returns the Shared field if non-nil, zero value otherwise.

### GetSharedOk

`func (o *MemoryZoneConfig) GetSharedOk() (*bool, bool)`

GetSharedOk returns a tuple with the Shared field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetShared

`func (o *MemoryZoneConfig) SetShared(v bool)`

SetShared sets Shared field to given value.

### HasShared

`func (o *MemoryZoneConfig) HasShared() bool`

HasShared returns a boolean if a field has been set.

### GetHugepages

`func (o *MemoryZoneConfig) GetHugepages() bool`

GetHugepages returns the Hugepages field if non-nil, zero value otherwise.

### GetHugepagesOk

`func (o *MemoryZoneConfig) GetHugepagesOk() (*bool, bool)`

GetHugepagesOk returns a tuple with the Hugepages field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHugepages

`func (o *MemoryZoneConfig) SetHugepages(v bool)`

SetHugepages sets Hugepages field to given value.

### HasHugepages

`func (o *MemoryZoneConfig) HasHugepages() bool`

HasHugepages returns a boolean if a field has been set.

### GetHugepageSize

`func (o *MemoryZoneConfig) GetHugepageSize() int64`

GetHugepageSize returns the HugepageSize field if non-nil, zero value otherwise.

### GetHugepageSizeOk

`func (o *MemoryZoneConfig) GetHugepageSizeOk() (*int64, bool)`

GetHugepageSizeOk returns a tuple with the HugepageSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHugepageSize

`func (o *MemoryZoneConfig) SetHugepageSize(v int64)`

SetHugepageSize sets HugepageSize field to given value.

### HasHugepageSize

`func (o *MemoryZoneConfig) HasHugepageSize() bool`

HasHugepageSize returns a boolean if a field has been set.

### GetHostNumaNode

`func (o *MemoryZoneConfig) GetHostNumaNode() int32`

GetHostNumaNode returns the HostNumaNode field if non-nil, zero value otherwise.

### GetHostNumaNodeOk

`func (o *MemoryZoneConfig) GetHostNumaNodeOk() (*int32, bool)`

GetHostNumaNodeOk returns a tuple with the HostNumaNode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHostNumaNode

`func (o *MemoryZoneConfig) SetHostNumaNode(v int32)`

SetHostNumaNode sets HostNumaNode field to given value.

### HasHostNumaNode

`func (o *MemoryZoneConfig) HasHostNumaNode() bool`

HasHostNumaNode returns a boolean if a field has been set.

### GetHotplugSize

`func (o *MemoryZoneConfig) GetHotplugSize() int64`

GetHotplugSize returns the HotplugSize field if non-nil, zero value otherwise.

### GetHotplugSizeOk

`func (o *MemoryZoneConfig) GetHotplugSizeOk() (*int64, bool)`

GetHotplugSizeOk returns a tuple with the HotplugSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHotplugSize

`func (o *MemoryZoneConfig) SetHotplugSize(v int64)`

SetHotplugSize sets HotplugSize field to given value.

### HasHotplugSize

`func (o *MemoryZoneConfig) HasHotplugSize() bool`

HasHotplugSize returns a boolean if a field has been set.

### GetHotpluggedSize

`func (o *MemoryZoneConfig) GetHotpluggedSize() int64`

GetHotpluggedSize returns the HotpluggedSize field if non-nil, zero value otherwise.

### GetHotpluggedSizeOk

`func (o *MemoryZoneConfig) GetHotpluggedSizeOk() (*int64, bool)`

GetHotpluggedSizeOk returns a tuple with the HotpluggedSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHotpluggedSize

`func (o *MemoryZoneConfig) SetHotpluggedSize(v int64)`

SetHotpluggedSize sets HotpluggedSize field to given value.

### HasHotpluggedSize

`func (o *MemoryZoneConfig) HasHotpluggedSize() bool`

HasHotpluggedSize returns a boolean if a field has been set.

### GetPrefault

`func (o *MemoryZoneConfig) GetPrefault() bool`

GetPrefault returns the Prefault field if non-nil, zero value otherwise.

### GetPrefaultOk

`func (o *MemoryZoneConfig) GetPrefaultOk() (*bool, bool)`

GetPrefaultOk returns a tuple with the Prefault field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPrefault

`func (o *MemoryZoneConfig) SetPrefault(v bool)`

SetPrefault sets Prefault field to given value.

### HasPrefault

`func (o *MemoryZoneConfig) HasPrefault() bool`

HasPrefault returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


