# RateLimitGroupConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | **string** |  | 
**RateLimiterConfig** | [**RateLimiterConfig**](RateLimiterConfig.md) |  | 

## Methods

### NewRateLimitGroupConfig

`func NewRateLimitGroupConfig(id string, rateLimiterConfig RateLimiterConfig, ) *RateLimitGroupConfig`

NewRateLimitGroupConfig instantiates a new RateLimitGroupConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewRateLimitGroupConfigWithDefaults

`func NewRateLimitGroupConfigWithDefaults() *RateLimitGroupConfig`

NewRateLimitGroupConfigWithDefaults instantiates a new RateLimitGroupConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *RateLimitGroupConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *RateLimitGroupConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *RateLimitGroupConfig) SetId(v string)`

SetId sets Id field to given value.


### GetRateLimiterConfig

`func (o *RateLimitGroupConfig) GetRateLimiterConfig() RateLimiterConfig`

GetRateLimiterConfig returns the RateLimiterConfig field if non-nil, zero value otherwise.

### GetRateLimiterConfigOk

`func (o *RateLimitGroupConfig) GetRateLimiterConfigOk() (*RateLimiterConfig, bool)`

GetRateLimiterConfigOk returns a tuple with the RateLimiterConfig field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetRateLimiterConfig

`func (o *RateLimitGroupConfig) SetRateLimiterConfig(v RateLimiterConfig)`

SetRateLimiterConfig sets RateLimiterConfig field to given value.



[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


