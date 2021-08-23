# CpuTopology

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ThreadsPerCore** | Pointer to **int32** |  | [optional] 
**CoresPerDie** | Pointer to **int32** |  | [optional] 
**DiesPerPackage** | Pointer to **int32** |  | [optional] 
**Packages** | Pointer to **int32** |  | [optional] 

## Methods

### NewCpuTopology

`func NewCpuTopology() *CpuTopology`

NewCpuTopology instantiates a new CpuTopology object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewCpuTopologyWithDefaults

`func NewCpuTopologyWithDefaults() *CpuTopology`

NewCpuTopologyWithDefaults instantiates a new CpuTopology object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetThreadsPerCore

`func (o *CpuTopology) GetThreadsPerCore() int32`

GetThreadsPerCore returns the ThreadsPerCore field if non-nil, zero value otherwise.

### GetThreadsPerCoreOk

`func (o *CpuTopology) GetThreadsPerCoreOk() (*int32, bool)`

GetThreadsPerCoreOk returns a tuple with the ThreadsPerCore field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetThreadsPerCore

`func (o *CpuTopology) SetThreadsPerCore(v int32)`

SetThreadsPerCore sets ThreadsPerCore field to given value.

### HasThreadsPerCore

`func (o *CpuTopology) HasThreadsPerCore() bool`

HasThreadsPerCore returns a boolean if a field has been set.

### GetCoresPerDie

`func (o *CpuTopology) GetCoresPerDie() int32`

GetCoresPerDie returns the CoresPerDie field if non-nil, zero value otherwise.

### GetCoresPerDieOk

`func (o *CpuTopology) GetCoresPerDieOk() (*int32, bool)`

GetCoresPerDieOk returns a tuple with the CoresPerDie field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetCoresPerDie

`func (o *CpuTopology) SetCoresPerDie(v int32)`

SetCoresPerDie sets CoresPerDie field to given value.

### HasCoresPerDie

`func (o *CpuTopology) HasCoresPerDie() bool`

HasCoresPerDie returns a boolean if a field has been set.

### GetDiesPerPackage

`func (o *CpuTopology) GetDiesPerPackage() int32`

GetDiesPerPackage returns the DiesPerPackage field if non-nil, zero value otherwise.

### GetDiesPerPackageOk

`func (o *CpuTopology) GetDiesPerPackageOk() (*int32, bool)`

GetDiesPerPackageOk returns a tuple with the DiesPerPackage field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDiesPerPackage

`func (o *CpuTopology) SetDiesPerPackage(v int32)`

SetDiesPerPackage sets DiesPerPackage field to given value.

### HasDiesPerPackage

`func (o *CpuTopology) HasDiesPerPackage() bool`

HasDiesPerPackage returns a boolean if a field has been set.

### GetPackages

`func (o *CpuTopology) GetPackages() int32`

GetPackages returns the Packages field if non-nil, zero value otherwise.

### GetPackagesOk

`func (o *CpuTopology) GetPackagesOk() (*int32, bool)`

GetPackagesOk returns a tuple with the Packages field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPackages

`func (o *CpuTopology) SetPackages(v int32)`

SetPackages sets Packages field to given value.

### HasPackages

`func (o *CpuTopology) HasPackages() bool`

HasPackages returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


