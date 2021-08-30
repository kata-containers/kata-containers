# RateLimiterConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Bandwidth** | Pointer to [**TokenBucket**](TokenBucket.md) |  | [optional] 
**Ops** | Pointer to [**TokenBucket**](TokenBucket.md) |  | [optional] 

## Methods

### NewRateLimiterConfig

`func NewRateLimiterConfig() *RateLimiterConfig`

NewRateLimiterConfig instantiates a new RateLimiterConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewRateLimiterConfigWithDefaults

`func NewRateLimiterConfigWithDefaults() *RateLimiterConfig`

NewRateLimiterConfigWithDefaults instantiates a new RateLimiterConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetBandwidth

`func (o *RateLimiterConfig) GetBandwidth() TokenBucket`

GetBandwidth returns the Bandwidth field if non-nil, zero value otherwise.

### GetBandwidthOk

`func (o *RateLimiterConfig) GetBandwidthOk() (*TokenBucket, bool)`

GetBandwidthOk returns a tuple with the Bandwidth field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetBandwidth

`func (o *RateLimiterConfig) SetBandwidth(v TokenBucket)`

SetBandwidth sets Bandwidth field to given value.

### HasBandwidth

`func (o *RateLimiterConfig) HasBandwidth() bool`

HasBandwidth returns a boolean if a field has been set.

### GetOps

`func (o *RateLimiterConfig) GetOps() TokenBucket`

GetOps returns the Ops field if non-nil, zero value otherwise.

### GetOpsOk

`func (o *RateLimiterConfig) GetOpsOk() (*TokenBucket, bool)`

GetOpsOk returns a tuple with the Ops field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetOps

`func (o *RateLimiterConfig) SetOps(v TokenBucket)`

SetOps sets Ops field to given value.

### HasOps

`func (o *RateLimiterConfig) HasOps() bool`

HasOps returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


