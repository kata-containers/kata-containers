# CpuAffinity

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Vcpu** | Pointer to **int32** |  | [optional] 
**HostCpus** | Pointer to **[]int32** |  | [optional] 

## Methods

### NewCpuAffinity

`func NewCpuAffinity() *CpuAffinity`

NewCpuAffinity instantiates a new CpuAffinity object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewCpuAffinityWithDefaults

`func NewCpuAffinityWithDefaults() *CpuAffinity`

NewCpuAffinityWithDefaults instantiates a new CpuAffinity object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetVcpu

`func (o *CpuAffinity) GetVcpu() int32`

GetVcpu returns the Vcpu field if non-nil, zero value otherwise.

### GetVcpuOk

`func (o *CpuAffinity) GetVcpuOk() (*int32, bool)`

GetVcpuOk returns a tuple with the Vcpu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVcpu

`func (o *CpuAffinity) SetVcpu(v int32)`

SetVcpu sets Vcpu field to given value.

### HasVcpu

`func (o *CpuAffinity) HasVcpu() bool`

HasVcpu returns a boolean if a field has been set.

### GetHostCpus

`func (o *CpuAffinity) GetHostCpus() []int32`

GetHostCpus returns the HostCpus field if non-nil, zero value otherwise.

### GetHostCpusOk

`func (o *CpuAffinity) GetHostCpusOk() (*[]int32, bool)`

GetHostCpusOk returns a tuple with the HostCpus field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHostCpus

`func (o *CpuAffinity) SetHostCpus(v []int32)`

SetHostCpus sets HostCpus field to given value.

### HasHostCpus

`func (o *CpuAffinity) HasHostCpus() bool`

HasHostCpus returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


