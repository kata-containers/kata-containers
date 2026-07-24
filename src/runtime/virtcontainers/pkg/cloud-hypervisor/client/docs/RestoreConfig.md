# RestoreConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**SourceUrl** | **string** |  |
**Prefault** | Pointer to **bool** |  | [optional]
**MemoryRestoreMode** | Pointer to [**MemoryRestoreMode**](MemoryRestoreMode.md) |  | [optional] [default to COPY]
**Resume** | Pointer to **bool** |  | [optional]

## Methods

### NewRestoreConfig

`func NewRestoreConfig(sourceUrl string, ) *RestoreConfig`

NewRestoreConfig instantiates a new RestoreConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewRestoreConfigWithDefaults

`func NewRestoreConfigWithDefaults() *RestoreConfig`

NewRestoreConfigWithDefaults instantiates a new RestoreConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSourceUrl

`func (o *RestoreConfig) GetSourceUrl() string`

GetSourceUrl returns the SourceUrl field if non-nil, zero value otherwise.

### GetSourceUrlOk

`func (o *RestoreConfig) GetSourceUrlOk() (*string, bool)`

GetSourceUrlOk returns a tuple with the SourceUrl field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSourceUrl

`func (o *RestoreConfig) SetSourceUrl(v string)`

SetSourceUrl sets SourceUrl field to given value.


### GetPrefault

`func (o *RestoreConfig) GetPrefault() bool`

GetPrefault returns the Prefault field if non-nil, zero value otherwise.

### GetPrefaultOk

`func (o *RestoreConfig) GetPrefaultOk() (*bool, bool)`

GetPrefaultOk returns a tuple with the Prefault field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPrefault

`func (o *RestoreConfig) SetPrefault(v bool)`

SetPrefault sets Prefault field to given value.

### HasPrefault

`func (o *RestoreConfig) HasPrefault() bool`

HasPrefault returns a boolean if a field has been set.

### GetMemoryRestoreMode

`func (o *RestoreConfig) GetMemoryRestoreMode() MemoryRestoreMode`

GetMemoryRestoreMode returns the MemoryRestoreMode field if non-nil, zero value otherwise.

### GetMemoryRestoreModeOk

`func (o *RestoreConfig) GetMemoryRestoreModeOk() (*MemoryRestoreMode, bool)`

GetMemoryRestoreModeOk returns a tuple with the MemoryRestoreMode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMemoryRestoreMode

`func (o *RestoreConfig) SetMemoryRestoreMode(v MemoryRestoreMode)`

SetMemoryRestoreMode sets MemoryRestoreMode field to given value.

### HasMemoryRestoreMode

`func (o *RestoreConfig) HasMemoryRestoreMode() bool`

HasMemoryRestoreMode returns a boolean if a field has been set.

### GetResume

`func (o *RestoreConfig) GetResume() bool`

GetResume returns the Resume field if non-nil, zero value otherwise.

### GetResumeOk

`func (o *RestoreConfig) GetResumeOk() (*bool, bool)`

GetResumeOk returns a tuple with the Resume field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetResume

`func (o *RestoreConfig) SetResume(v bool)`

SetResume sets Resume field to given value.

### HasResume

`func (o *RestoreConfig) HasResume() bool`

HasResume returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
