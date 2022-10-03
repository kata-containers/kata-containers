# NetConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Tap** | Pointer to **string** |  | [optional] 
**Ip** | Pointer to **string** |  | [optional] [default to "192.168.249.1"]
**Mask** | Pointer to **string** |  | [optional] [default to "255.255.255.0"]
**Mac** | Pointer to **string** |  | [optional] 
**HostMac** | Pointer to **string** |  | [optional] 
**Mtu** | Pointer to **int32** |  | [optional] 
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**NumQueues** | Pointer to **int32** |  | [optional] [default to 2]
**QueueSize** | Pointer to **int32** |  | [optional] [default to 256]
**VhostUser** | Pointer to **bool** |  | [optional] [default to false]
**VhostSocket** | Pointer to **string** |  | [optional] 
**VhostMode** | Pointer to **string** |  | [optional] [default to "Client"]
**Id** | Pointer to **string** |  | [optional] 
**PciSegment** | Pointer to **int32** |  | [optional] 
**RateLimiterConfig** | Pointer to [**RateLimiterConfig**](RateLimiterConfig.md) |  | [optional] 

## Methods

### NewNetConfig

`func NewNetConfig() *NetConfig`

NewNetConfig instantiates a new NetConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewNetConfigWithDefaults

`func NewNetConfigWithDefaults() *NetConfig`

NewNetConfigWithDefaults instantiates a new NetConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetTap

`func (o *NetConfig) GetTap() string`

GetTap returns the Tap field if non-nil, zero value otherwise.

### GetTapOk

`func (o *NetConfig) GetTapOk() (*string, bool)`

GetTapOk returns a tuple with the Tap field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTap

`func (o *NetConfig) SetTap(v string)`

SetTap sets Tap field to given value.

### HasTap

`func (o *NetConfig) HasTap() bool`

HasTap returns a boolean if a field has been set.

### GetIp

`func (o *NetConfig) GetIp() string`

GetIp returns the Ip field if non-nil, zero value otherwise.

### GetIpOk

`func (o *NetConfig) GetIpOk() (*string, bool)`

GetIpOk returns a tuple with the Ip field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIp

`func (o *NetConfig) SetIp(v string)`

SetIp sets Ip field to given value.

### HasIp

`func (o *NetConfig) HasIp() bool`

HasIp returns a boolean if a field has been set.

### GetMask

`func (o *NetConfig) GetMask() string`

GetMask returns the Mask field if non-nil, zero value otherwise.

### GetMaskOk

`func (o *NetConfig) GetMaskOk() (*string, bool)`

GetMaskOk returns a tuple with the Mask field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMask

`func (o *NetConfig) SetMask(v string)`

SetMask sets Mask field to given value.

### HasMask

`func (o *NetConfig) HasMask() bool`

HasMask returns a boolean if a field has been set.

### GetMac

`func (o *NetConfig) GetMac() string`

GetMac returns the Mac field if non-nil, zero value otherwise.

### GetMacOk

`func (o *NetConfig) GetMacOk() (*string, bool)`

GetMacOk returns a tuple with the Mac field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMac

`func (o *NetConfig) SetMac(v string)`

SetMac sets Mac field to given value.

### HasMac

`func (o *NetConfig) HasMac() bool`

HasMac returns a boolean if a field has been set.

### GetHostMac

`func (o *NetConfig) GetHostMac() string`

GetHostMac returns the HostMac field if non-nil, zero value otherwise.

### GetHostMacOk

`func (o *NetConfig) GetHostMacOk() (*string, bool)`

GetHostMacOk returns a tuple with the HostMac field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHostMac

`func (o *NetConfig) SetHostMac(v string)`

SetHostMac sets HostMac field to given value.

### HasHostMac

`func (o *NetConfig) HasHostMac() bool`

HasHostMac returns a boolean if a field has been set.

### GetMtu

`func (o *NetConfig) GetMtu() int32`

GetMtu returns the Mtu field if non-nil, zero value otherwise.

### GetMtuOk

`func (o *NetConfig) GetMtuOk() (*int32, bool)`

GetMtuOk returns a tuple with the Mtu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMtu

`func (o *NetConfig) SetMtu(v int32)`

SetMtu sets Mtu field to given value.

### HasMtu

`func (o *NetConfig) HasMtu() bool`

HasMtu returns a boolean if a field has been set.

### GetIommu

`func (o *NetConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *NetConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *NetConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *NetConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.

### GetNumQueues

`func (o *NetConfig) GetNumQueues() int32`

GetNumQueues returns the NumQueues field if non-nil, zero value otherwise.

### GetNumQueuesOk

`func (o *NetConfig) GetNumQueuesOk() (*int32, bool)`

GetNumQueuesOk returns a tuple with the NumQueues field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetNumQueues

`func (o *NetConfig) SetNumQueues(v int32)`

SetNumQueues sets NumQueues field to given value.

### HasNumQueues

`func (o *NetConfig) HasNumQueues() bool`

HasNumQueues returns a boolean if a field has been set.

### GetQueueSize

`func (o *NetConfig) GetQueueSize() int32`

GetQueueSize returns the QueueSize field if non-nil, zero value otherwise.

### GetQueueSizeOk

`func (o *NetConfig) GetQueueSizeOk() (*int32, bool)`

GetQueueSizeOk returns a tuple with the QueueSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetQueueSize

`func (o *NetConfig) SetQueueSize(v int32)`

SetQueueSize sets QueueSize field to given value.

### HasQueueSize

`func (o *NetConfig) HasQueueSize() bool`

HasQueueSize returns a boolean if a field has been set.

### GetVhostUser

`func (o *NetConfig) GetVhostUser() bool`

GetVhostUser returns the VhostUser field if non-nil, zero value otherwise.

### GetVhostUserOk

`func (o *NetConfig) GetVhostUserOk() (*bool, bool)`

GetVhostUserOk returns a tuple with the VhostUser field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVhostUser

`func (o *NetConfig) SetVhostUser(v bool)`

SetVhostUser sets VhostUser field to given value.

### HasVhostUser

`func (o *NetConfig) HasVhostUser() bool`

HasVhostUser returns a boolean if a field has been set.

### GetVhostSocket

`func (o *NetConfig) GetVhostSocket() string`

GetVhostSocket returns the VhostSocket field if non-nil, zero value otherwise.

### GetVhostSocketOk

`func (o *NetConfig) GetVhostSocketOk() (*string, bool)`

GetVhostSocketOk returns a tuple with the VhostSocket field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVhostSocket

`func (o *NetConfig) SetVhostSocket(v string)`

SetVhostSocket sets VhostSocket field to given value.

### HasVhostSocket

`func (o *NetConfig) HasVhostSocket() bool`

HasVhostSocket returns a boolean if a field has been set.

### GetVhostMode

`func (o *NetConfig) GetVhostMode() string`

GetVhostMode returns the VhostMode field if non-nil, zero value otherwise.

### GetVhostModeOk

`func (o *NetConfig) GetVhostModeOk() (*string, bool)`

GetVhostModeOk returns a tuple with the VhostMode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVhostMode

`func (o *NetConfig) SetVhostMode(v string)`

SetVhostMode sets VhostMode field to given value.

### HasVhostMode

`func (o *NetConfig) HasVhostMode() bool`

HasVhostMode returns a boolean if a field has been set.

### GetId

`func (o *NetConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *NetConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *NetConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *NetConfig) HasId() bool`

HasId returns a boolean if a field has been set.

### GetPciSegment

`func (o *NetConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *NetConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *NetConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *NetConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetRateLimiterConfig

`func (o *NetConfig) GetRateLimiterConfig() RateLimiterConfig`

GetRateLimiterConfig returns the RateLimiterConfig field if non-nil, zero value otherwise.

### GetRateLimiterConfigOk

`func (o *NetConfig) GetRateLimiterConfigOk() (*RateLimiterConfig, bool)`

GetRateLimiterConfigOk returns a tuple with the RateLimiterConfig field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetRateLimiterConfig

`func (o *NetConfig) SetRateLimiterConfig(v RateLimiterConfig)`

SetRateLimiterConfig sets RateLimiterConfig field to given value.

### HasRateLimiterConfig

`func (o *NetConfig) HasRateLimiterConfig() bool`

HasRateLimiterConfig returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


