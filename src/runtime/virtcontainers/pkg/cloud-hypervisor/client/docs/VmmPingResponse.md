# VmmPingResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**BuildVersion** | Pointer to **string** |  | [optional] 
**Version** | **string** |  | 
**Pid** | Pointer to **int64** |  | [optional] 
**Features** | Pointer to **[]string** |  | [optional] 

## Methods

### NewVmmPingResponse

`func NewVmmPingResponse(version string, ) *VmmPingResponse`

NewVmmPingResponse instantiates a new VmmPingResponse object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVmmPingResponseWithDefaults

`func NewVmmPingResponseWithDefaults() *VmmPingResponse`

NewVmmPingResponseWithDefaults instantiates a new VmmPingResponse object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetBuildVersion

`func (o *VmmPingResponse) GetBuildVersion() string`

GetBuildVersion returns the BuildVersion field if non-nil, zero value otherwise.

### GetBuildVersionOk

`func (o *VmmPingResponse) GetBuildVersionOk() (*string, bool)`

GetBuildVersionOk returns a tuple with the BuildVersion field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetBuildVersion

`func (o *VmmPingResponse) SetBuildVersion(v string)`

SetBuildVersion sets BuildVersion field to given value.

### HasBuildVersion

`func (o *VmmPingResponse) HasBuildVersion() bool`

HasBuildVersion returns a boolean if a field has been set.

### GetVersion

`func (o *VmmPingResponse) GetVersion() string`

GetVersion returns the Version field if non-nil, zero value otherwise.

### GetVersionOk

`func (o *VmmPingResponse) GetVersionOk() (*string, bool)`

GetVersionOk returns a tuple with the Version field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVersion

`func (o *VmmPingResponse) SetVersion(v string)`

SetVersion sets Version field to given value.


### GetPid

`func (o *VmmPingResponse) GetPid() int64`

GetPid returns the Pid field if non-nil, zero value otherwise.

### GetPidOk

`func (o *VmmPingResponse) GetPidOk() (*int64, bool)`

GetPidOk returns a tuple with the Pid field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPid

`func (o *VmmPingResponse) SetPid(v int64)`

SetPid sets Pid field to given value.

### HasPid

`func (o *VmmPingResponse) HasPid() bool`

HasPid returns a boolean if a field has been set.

### GetFeatures

`func (o *VmmPingResponse) GetFeatures() []string`

GetFeatures returns the Features field if non-nil, zero value otherwise.

### GetFeaturesOk

`func (o *VmmPingResponse) GetFeaturesOk() (*[]string, bool)`

GetFeaturesOk returns a tuple with the Features field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFeatures

`func (o *VmmPingResponse) SetFeatures(v []string)`

SetFeatures sets Features field to given value.

### HasFeatures

`func (o *VmmPingResponse) HasFeatures() bool`

HasFeatures returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


