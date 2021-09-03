# CpusConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**BootVcpus** | **int32** |  | [default to 1]
**MaxVcpus** | **int32** |  | [default to 1]
**Topology** | Pointer to [**CpuTopology**](CpuTopology.md) |  | [optional] 
**MaxPhysBits** | Pointer to **int32** |  | [optional] 

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


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


