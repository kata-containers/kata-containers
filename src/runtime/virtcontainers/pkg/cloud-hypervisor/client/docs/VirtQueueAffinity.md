# VirtQueueAffinity

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**QueueIndex** | **int32** |  | 
**HostCpus** | **[]int32** |  | 

## Methods

### NewVirtQueueAffinity

`func NewVirtQueueAffinity(queueIndex int32, hostCpus []int32, ) *VirtQueueAffinity`

NewVirtQueueAffinity instantiates a new VirtQueueAffinity object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVirtQueueAffinityWithDefaults

`func NewVirtQueueAffinityWithDefaults() *VirtQueueAffinity`

NewVirtQueueAffinityWithDefaults instantiates a new VirtQueueAffinity object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetQueueIndex

`func (o *VirtQueueAffinity) GetQueueIndex() int32`

GetQueueIndex returns the QueueIndex field if non-nil, zero value otherwise.

### GetQueueIndexOk

`func (o *VirtQueueAffinity) GetQueueIndexOk() (*int32, bool)`

GetQueueIndexOk returns a tuple with the QueueIndex field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetQueueIndex

`func (o *VirtQueueAffinity) SetQueueIndex(v int32)`

SetQueueIndex sets QueueIndex field to given value.


### GetHostCpus

`func (o *VirtQueueAffinity) GetHostCpus() []int32`

GetHostCpus returns the HostCpus field if non-nil, zero value otherwise.

### GetHostCpusOk

`func (o *VirtQueueAffinity) GetHostCpusOk() (*[]int32, bool)`

GetHostCpusOk returns a tuple with the HostCpus field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHostCpus

`func (o *VirtQueueAffinity) SetHostCpus(v []int32)`

SetHostCpus sets HostCpus field to given value.



[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


