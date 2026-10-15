# BalloonStatsResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**BalloonActual** | **int64** | Current balloon size in bytes |
**LastUpdate** | **int64** | Host UNIX timestamp in milliseconds when statistics were last updated, or zero before the first sample |
**Stats** | [**BalloonStats**](BalloonStats.md) |  |

## Methods

### NewBalloonStatsResponse

`func NewBalloonStatsResponse(balloonActual int64, lastUpdate int64, stats BalloonStats, ) *BalloonStatsResponse`

NewBalloonStatsResponse instantiates a new BalloonStatsResponse object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewBalloonStatsResponseWithDefaults

`func NewBalloonStatsResponseWithDefaults() *BalloonStatsResponse`

NewBalloonStatsResponseWithDefaults instantiates a new BalloonStatsResponse object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetBalloonActual

`func (o *BalloonStatsResponse) GetBalloonActual() int64`

GetBalloonActual returns the BalloonActual field if non-nil, zero value otherwise.

### GetBalloonActualOk

`func (o *BalloonStatsResponse) GetBalloonActualOk() (*int64, bool)`

GetBalloonActualOk returns a tuple with the BalloonActual field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetBalloonActual

`func (o *BalloonStatsResponse) SetBalloonActual(v int64)`

SetBalloonActual sets BalloonActual field to given value.


### GetLastUpdate

`func (o *BalloonStatsResponse) GetLastUpdate() int64`

GetLastUpdate returns the LastUpdate field if non-nil, zero value otherwise.

### GetLastUpdateOk

`func (o *BalloonStatsResponse) GetLastUpdateOk() (*int64, bool)`

GetLastUpdateOk returns a tuple with the LastUpdate field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetLastUpdate

`func (o *BalloonStatsResponse) SetLastUpdate(v int64)`

SetLastUpdate sets LastUpdate field to given value.


### GetStats

`func (o *BalloonStatsResponse) GetStats() BalloonStats`

GetStats returns the Stats field if non-nil, zero value otherwise.

### GetStatsOk

`func (o *BalloonStatsResponse) GetStatsOk() (*BalloonStats, bool)`

GetStatsOk returns a tuple with the Stats field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetStats

`func (o *BalloonStatsResponse) SetStats(v BalloonStats)`

SetStats sets Stats field to given value.



[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
