# TokenBucket

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Size** | **int64** | The total number of tokens this bucket can hold. | 
**OneTimeBurst** | Pointer to **int64** | The initial size of a token bucket. | [optional] 
**RefillTime** | **int64** | The amount of milliseconds it takes for the bucket to refill. | 

## Methods

### NewTokenBucket

`func NewTokenBucket(size int64, refillTime int64, ) *TokenBucket`

NewTokenBucket instantiates a new TokenBucket object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewTokenBucketWithDefaults

`func NewTokenBucketWithDefaults() *TokenBucket`

NewTokenBucketWithDefaults instantiates a new TokenBucket object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSize

`func (o *TokenBucket) GetSize() int64`

GetSize returns the Size field if non-nil, zero value otherwise.

### GetSizeOk

`func (o *TokenBucket) GetSizeOk() (*int64, bool)`

GetSizeOk returns a tuple with the Size field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSize

`func (o *TokenBucket) SetSize(v int64)`

SetSize sets Size field to given value.


### GetOneTimeBurst

`func (o *TokenBucket) GetOneTimeBurst() int64`

GetOneTimeBurst returns the OneTimeBurst field if non-nil, zero value otherwise.

### GetOneTimeBurstOk

`func (o *TokenBucket) GetOneTimeBurstOk() (*int64, bool)`

GetOneTimeBurstOk returns a tuple with the OneTimeBurst field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetOneTimeBurst

`func (o *TokenBucket) SetOneTimeBurst(v int64)`

SetOneTimeBurst sets OneTimeBurst field to given value.

### HasOneTimeBurst

`func (o *TokenBucket) HasOneTimeBurst() bool`

HasOneTimeBurst returns a boolean if a field has been set.

### GetRefillTime

`func (o *TokenBucket) GetRefillTime() int64`

GetRefillTime returns the RefillTime field if non-nil, zero value otherwise.

### GetRefillTimeOk

`func (o *TokenBucket) GetRefillTimeOk() (*int64, bool)`

GetRefillTimeOk returns a tuple with the RefillTime field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetRefillTime

`func (o *TokenBucket) SetRefillTime(v int64)`

SetRefillTime sets RefillTime field to given value.



[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


