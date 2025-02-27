# CpusConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**BootVcpus** | **int32** |  | 
**MaxVcpus** | **int32** |  | 
**Topology** | Pointer to [**CpuTopology**](CpuTopology.md) |  | [optional] 
**KvmHyperv** | Pointer to **bool** |  | [optional] [default to false]
**MaxPhysBits** | Pointer to **int32** |  | [optional] 
**Affinity** | Pointer to [**[]CpuAffinity**](CpuAffinity.md) |  | [optional] 
**Features** | Pointer to [**CpuFeatures**](CpuFeatures.md) |  | [optional] 

## Methods

### NewCpusConfig

`func NewCpusConfig(bootVcpus int32, maxVcpus int32, ) *CpusConfig`

NewCpusConfig instantiates a new CpusConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewCpusConfigWithDefaults

`func NewCpusConfigWithDefaults() *CpusConfig`

NewCpusConfigWithDefaults instantiates a new CpusConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetBootVcpus

`func (o *CpusConfig) GetBootVcpus() int32`

GetBootVcpus returns the BootVcpus field if non-nil, zero value otherwise.

### GetBootVcpusOk

`func (o *CpusConfig) GetBootVcpusOk() (*int32, bool)`

GetBootVcpusOk returns a tuple with the BootVcpus field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetBootVcpus

`func (o *CpusConfig) SetBootVcpus(v int32)`

SetBootVcpus sets BootVcpus field to given value.


### GetMaxVcpus

`func (o *CpusConfig) GetMaxVcpus() int32`

GetMaxVcpus returns the MaxVcpus field if non-nil, zero value otherwise.

### GetMaxVcpusOk

`func (o *CpusConfig) GetMaxVcpusOk() (*int32, bool)`

GetMaxVcpusOk returns a tuple with the MaxVcpus field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMaxVcpus

`func (o *CpusConfig) SetMaxVcpus(v int32)`

SetMaxVcpus sets MaxVcpus field to given value.


### GetTopology

`func (o *CpusConfig) GetTopology() CpuTopology`

GetTopology returns the Topology field if non-nil, zero value otherwise.

### GetTopologyOk

`func (o *CpusConfig) GetTopologyOk() (*CpuTopology, bool)`

GetTopologyOk returns a tuple with the Topology field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTopology

`func (o *CpusConfig) SetTopology(v CpuTopology)`

SetTopology sets Topology field to given value.

### HasTopology

`func (o *CpusConfig) HasTopology() bool`

HasTopology returns a boolean if a field has been set.

### GetKvmHyperv

`func (o *CpusConfig) GetKvmHyperv() bool`

GetKvmHyperv returns the KvmHyperv field if non-nil, zero value otherwise.

### GetKvmHypervOk

`func (o *CpusConfig) GetKvmHypervOk() (*bool, bool)`

GetKvmHypervOk returns a tuple with the KvmHyperv field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetKvmHyperv

`func (o *CpusConfig) SetKvmHyperv(v bool)`

SetKvmHyperv sets KvmHyperv field to given value.

### HasKvmHyperv

`func (o *CpusConfig) HasKvmHyperv() bool`

HasKvmHyperv returns a boolean if a field has been set.

### GetMaxPhysBits

`func (o *CpusConfig) GetMaxPhysBits() int32`

GetMaxPhysBits returns the MaxPhysBits field if non-nil, zero value otherwise.

### GetMaxPhysBitsOk

`func (o *CpusConfig) GetMaxPhysBitsOk() (*int32, bool)`

GetMaxPhysBitsOk returns a tuple with the MaxPhysBits field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMaxPhysBits

`func (o *CpusConfig) SetMaxPhysBits(v int32)`

SetMaxPhysBits sets MaxPhysBits field to given value.

### HasMaxPhysBits

`func (o *CpusConfig) HasMaxPhysBits() bool`

HasMaxPhysBits returns a boolean if a field has been set.

### GetAffinity

`func (o *CpusConfig) GetAffinity() []CpuAffinity`

GetAffinity returns the Affinity field if non-nil, zero value otherwise.

### GetAffinityOk

`func (o *CpusConfig) GetAffinityOk() (*[]CpuAffinity, bool)`

GetAffinityOk returns a tuple with the Affinity field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetAffinity

`func (o *CpusConfig) SetAffinity(v []CpuAffinity)`

SetAffinity sets Affinity field to given value.

### HasAffinity

`func (o *CpusConfig) HasAffinity() bool`

HasAffinity returns a boolean if a field has been set.

### GetFeatures

`func (o *CpusConfig) GetFeatures() CpuFeatures`

GetFeatures returns the Features field if non-nil, zero value otherwise.

### GetFeaturesOk

`func (o *CpusConfig) GetFeaturesOk() (*CpuFeatures, bool)`

GetFeaturesOk returns a tuple with the Features field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFeatures

`func (o *CpusConfig) SetFeatures(v CpuFeatures)`

SetFeatures sets Features field to given value.

### HasFeatures

`func (o *CpusConfig) HasFeatures() bool`

HasFeatures returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


