# DiskConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Path** | Pointer to **string** |  | [optional]
**Readonly** | Pointer to **bool** |  | [optional] [default to false]
**Direct** | Pointer to **bool** |  | [optional] [default to false]
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**NumQueues** | Pointer to **int32** |  | [optional] [default to 1]
**QueueSize** | Pointer to **int32** |  | [optional] [default to 128]
**VhostUser** | Pointer to **bool** |  | [optional] [default to false]
**VhostSocket** | Pointer to **string** |  | [optional]
**RateLimiterConfig** | Pointer to [**RateLimiterConfig**](RateLimiterConfig.md) |  | [optional]
**PciSegment** | Pointer to **int32** |  | [optional]
**PciDeviceId** | Pointer to **int32** |  | [optional]
**Id** | Pointer to **string** |  | [optional]
**Serial** | Pointer to **string** |  | [optional]
**RateLimitGroup** | Pointer to **string** |  | [optional]
**QueueAffinity** | Pointer to [**[]VirtQueueAffinity**](VirtQueueAffinity.md) |  | [optional]
**BackingFiles** | Pointer to **bool** |  | [optional] [default to false]
**Sparse** | Pointer to **bool** |  | [optional] [default to true]
**ImageType** | Pointer to [**ImageType**](ImageType.md) |  | [optional]
**LockGranularity** | Pointer to [**LockGranularity**](LockGranularity.md) |  | [optional] [default to BYTE_RANGE]

## Methods

### NewDiskConfig

`func NewDiskConfig() *DiskConfig`

NewDiskConfig instantiates a new DiskConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewDiskConfigWithDefaults

`func NewDiskConfigWithDefaults() *DiskConfig`

NewDiskConfigWithDefaults instantiates a new DiskConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetPath

`func (o *DiskConfig) GetPath() string`

GetPath returns the Path field if non-nil, zero value otherwise.

### GetPathOk

`func (o *DiskConfig) GetPathOk() (*string, bool)`

GetPathOk returns a tuple with the Path field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPath

`func (o *DiskConfig) SetPath(v string)`

SetPath sets Path field to given value.

### HasPath

`func (o *DiskConfig) HasPath() bool`

HasPath returns a boolean if a field has been set.

### GetReadonly

`func (o *DiskConfig) GetReadonly() bool`

GetReadonly returns the Readonly field if non-nil, zero value otherwise.

### GetReadonlyOk

`func (o *DiskConfig) GetReadonlyOk() (*bool, bool)`

GetReadonlyOk returns a tuple with the Readonly field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetReadonly

`func (o *DiskConfig) SetReadonly(v bool)`

SetReadonly sets Readonly field to given value.

### HasReadonly

`func (o *DiskConfig) HasReadonly() bool`

HasReadonly returns a boolean if a field has been set.

### GetDirect

`func (o *DiskConfig) GetDirect() bool`

GetDirect returns the Direct field if non-nil, zero value otherwise.

### GetDirectOk

`func (o *DiskConfig) GetDirectOk() (*bool, bool)`

GetDirectOk returns a tuple with the Direct field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDirect

`func (o *DiskConfig) SetDirect(v bool)`

SetDirect sets Direct field to given value.

### HasDirect

`func (o *DiskConfig) HasDirect() bool`

HasDirect returns a boolean if a field has been set.

### GetIommu

`func (o *DiskConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *DiskConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *DiskConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *DiskConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.

### GetNumQueues

`func (o *DiskConfig) GetNumQueues() int32`

GetNumQueues returns the NumQueues field if non-nil, zero value otherwise.

### GetNumQueuesOk

`func (o *DiskConfig) GetNumQueuesOk() (*int32, bool)`

GetNumQueuesOk returns a tuple with the NumQueues field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetNumQueues

`func (o *DiskConfig) SetNumQueues(v int32)`

SetNumQueues sets NumQueues field to given value.

### HasNumQueues

`func (o *DiskConfig) HasNumQueues() bool`

HasNumQueues returns a boolean if a field has been set.

### GetQueueSize

`func (o *DiskConfig) GetQueueSize() int32`

GetQueueSize returns the QueueSize field if non-nil, zero value otherwise.

### GetQueueSizeOk

`func (o *DiskConfig) GetQueueSizeOk() (*int32, bool)`

GetQueueSizeOk returns a tuple with the QueueSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetQueueSize

`func (o *DiskConfig) SetQueueSize(v int32)`

SetQueueSize sets QueueSize field to given value.

### HasQueueSize

`func (o *DiskConfig) HasQueueSize() bool`

HasQueueSize returns a boolean if a field has been set.

### GetVhostUser

`func (o *DiskConfig) GetVhostUser() bool`

GetVhostUser returns the VhostUser field if non-nil, zero value otherwise.

### GetVhostUserOk

`func (o *DiskConfig) GetVhostUserOk() (*bool, bool)`

GetVhostUserOk returns a tuple with the VhostUser field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVhostUser

`func (o *DiskConfig) SetVhostUser(v bool)`

SetVhostUser sets VhostUser field to given value.

### HasVhostUser

`func (o *DiskConfig) HasVhostUser() bool`

HasVhostUser returns a boolean if a field has been set.

### GetVhostSocket

`func (o *DiskConfig) GetVhostSocket() string`

GetVhostSocket returns the VhostSocket field if non-nil, zero value otherwise.

### GetVhostSocketOk

`func (o *DiskConfig) GetVhostSocketOk() (*string, bool)`

GetVhostSocketOk returns a tuple with the VhostSocket field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVhostSocket

`func (o *DiskConfig) SetVhostSocket(v string)`

SetVhostSocket sets VhostSocket field to given value.

### HasVhostSocket

`func (o *DiskConfig) HasVhostSocket() bool`

HasVhostSocket returns a boolean if a field has been set.

### GetRateLimiterConfig

`func (o *DiskConfig) GetRateLimiterConfig() RateLimiterConfig`

GetRateLimiterConfig returns the RateLimiterConfig field if non-nil, zero value otherwise.

### GetRateLimiterConfigOk

`func (o *DiskConfig) GetRateLimiterConfigOk() (*RateLimiterConfig, bool)`

GetRateLimiterConfigOk returns a tuple with the RateLimiterConfig field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetRateLimiterConfig

`func (o *DiskConfig) SetRateLimiterConfig(v RateLimiterConfig)`

SetRateLimiterConfig sets RateLimiterConfig field to given value.

### HasRateLimiterConfig

`func (o *DiskConfig) HasRateLimiterConfig() bool`

HasRateLimiterConfig returns a boolean if a field has been set.

### GetPciSegment

`func (o *DiskConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *DiskConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *DiskConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *DiskConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetPciDeviceId

`func (o *DiskConfig) GetPciDeviceId() int32`

GetPciDeviceId returns the PciDeviceId field if non-nil, zero value otherwise.

### GetPciDeviceIdOk

`func (o *DiskConfig) GetPciDeviceIdOk() (*int32, bool)`

GetPciDeviceIdOk returns a tuple with the PciDeviceId field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciDeviceId

`func (o *DiskConfig) SetPciDeviceId(v int32)`

SetPciDeviceId sets PciDeviceId field to given value.

### HasPciDeviceId

`func (o *DiskConfig) HasPciDeviceId() bool`

HasPciDeviceId returns a boolean if a field has been set.

### GetId

`func (o *DiskConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *DiskConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *DiskConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *DiskConfig) HasId() bool`

HasId returns a boolean if a field has been set.

### GetSerial

`func (o *DiskConfig) GetSerial() string`

GetSerial returns the Serial field if non-nil, zero value otherwise.

### GetSerialOk

`func (o *DiskConfig) GetSerialOk() (*string, bool)`

GetSerialOk returns a tuple with the Serial field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSerial

`func (o *DiskConfig) SetSerial(v string)`

SetSerial sets Serial field to given value.

### HasSerial

`func (o *DiskConfig) HasSerial() bool`

HasSerial returns a boolean if a field has been set.

### GetRateLimitGroup

`func (o *DiskConfig) GetRateLimitGroup() string`

GetRateLimitGroup returns the RateLimitGroup field if non-nil, zero value otherwise.

### GetRateLimitGroupOk

`func (o *DiskConfig) GetRateLimitGroupOk() (*string, bool)`

GetRateLimitGroupOk returns a tuple with the RateLimitGroup field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetRateLimitGroup

`func (o *DiskConfig) SetRateLimitGroup(v string)`

SetRateLimitGroup sets RateLimitGroup field to given value.

### HasRateLimitGroup

`func (o *DiskConfig) HasRateLimitGroup() bool`

HasRateLimitGroup returns a boolean if a field has been set.

### GetQueueAffinity

`func (o *DiskConfig) GetQueueAffinity() []VirtQueueAffinity`

GetQueueAffinity returns the QueueAffinity field if non-nil, zero value otherwise.

### GetQueueAffinityOk

`func (o *DiskConfig) GetQueueAffinityOk() (*[]VirtQueueAffinity, bool)`

GetQueueAffinityOk returns a tuple with the QueueAffinity field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetQueueAffinity

`func (o *DiskConfig) SetQueueAffinity(v []VirtQueueAffinity)`

SetQueueAffinity sets QueueAffinity field to given value.

### HasQueueAffinity

`func (o *DiskConfig) HasQueueAffinity() bool`

HasQueueAffinity returns a boolean if a field has been set.

### GetBackingFiles

`func (o *DiskConfig) GetBackingFiles() bool`

GetBackingFiles returns the BackingFiles field if non-nil, zero value otherwise.

### GetBackingFilesOk

`func (o *DiskConfig) GetBackingFilesOk() (*bool, bool)`

GetBackingFilesOk returns a tuple with the BackingFiles field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetBackingFiles

`func (o *DiskConfig) SetBackingFiles(v bool)`

SetBackingFiles sets BackingFiles field to given value.

### HasBackingFiles

`func (o *DiskConfig) HasBackingFiles() bool`

HasBackingFiles returns a boolean if a field has been set.

### GetSparse

`func (o *DiskConfig) GetSparse() bool`

GetSparse returns the Sparse field if non-nil, zero value otherwise.

### GetSparseOk

`func (o *DiskConfig) GetSparseOk() (*bool, bool)`

GetSparseOk returns a tuple with the Sparse field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSparse

`func (o *DiskConfig) SetSparse(v bool)`

SetSparse sets Sparse field to given value.

### HasSparse

`func (o *DiskConfig) HasSparse() bool`

HasSparse returns a boolean if a field has been set.

### GetImageType

`func (o *DiskConfig) GetImageType() ImageType`

GetImageType returns the ImageType field if non-nil, zero value otherwise.

### GetImageTypeOk

`func (o *DiskConfig) GetImageTypeOk() (*ImageType, bool)`

GetImageTypeOk returns a tuple with the ImageType field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetImageType

`func (o *DiskConfig) SetImageType(v ImageType)`

SetImageType sets ImageType field to given value.

### HasImageType

`func (o *DiskConfig) HasImageType() bool`

HasImageType returns a boolean if a field has been set.

### GetLockGranularity

`func (o *DiskConfig) GetLockGranularity() LockGranularity`

GetLockGranularity returns the LockGranularity field if non-nil, zero value otherwise.

### GetLockGranularityOk

`func (o *DiskConfig) GetLockGranularityOk() (*LockGranularity, bool)`

GetLockGranularityOk returns a tuple with the LockGranularity field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetLockGranularity

`func (o *DiskConfig) SetLockGranularity(v LockGranularity)`

SetLockGranularity sets LockGranularity field to given value.

### HasLockGranularity

`func (o *DiskConfig) HasLockGranularity() bool`

HasLockGranularity returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
