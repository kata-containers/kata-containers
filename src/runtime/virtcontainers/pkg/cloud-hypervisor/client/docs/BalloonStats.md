# BalloonStats

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**SwapIn** | Pointer to **int64** | Memory swapped into the guest in bytes | [optional]
**SwapOut** | Pointer to **int64** | Memory swapped out by the guest in bytes | [optional]
**MajorFaults** | Pointer to **int64** | Number of major page faults | [optional]
**MinorFaults** | Pointer to **int64** | Number of minor page faults | [optional]
**FreeMemory** | Pointer to **int64** | Unused guest memory in bytes | [optional]
**TotalMemory** | Pointer to **int64** | Total guest memory in bytes | [optional]
**AvailableMemory** | Pointer to **int64** | Guest memory available without swapping in bytes | [optional]
**DiskCaches** | Pointer to **int64** | Reclaimable guest disk caches in bytes | [optional]
**HugetlbAllocations** | Pointer to **int64** | Number of successful hugetlb allocations | [optional]
**HugetlbFailures** | Pointer to **int64** | Number of failed hugetlb allocations | [optional]
**OomKills** | Pointer to **int64** | Number of guest OOM kills reported by Linux | [optional]
**AllocStalls** | Pointer to **int64** | Number of guest memory allocation stalls reported by Linux | [optional]
**AsyncScans** | Pointer to **int64** | Memory scanned asynchronously by Linux in bytes | [optional]
**DirectScans** | Pointer to **int64** | Memory scanned directly by Linux in bytes | [optional]
**AsyncReclaims** | Pointer to **int64** | Memory reclaimed asynchronously by Linux in bytes | [optional]
**DirectReclaims** | Pointer to **int64** | Memory reclaimed directly by Linux in bytes | [optional]

## Methods

### NewBalloonStats

`func NewBalloonStats() *BalloonStats`

NewBalloonStats instantiates a new BalloonStats object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewBalloonStatsWithDefaults

`func NewBalloonStatsWithDefaults() *BalloonStats`

NewBalloonStatsWithDefaults instantiates a new BalloonStats object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSwapIn

`func (o *BalloonStats) GetSwapIn() int64`

GetSwapIn returns the SwapIn field if non-nil, zero value otherwise.

### GetSwapInOk

`func (o *BalloonStats) GetSwapInOk() (*int64, bool)`

GetSwapInOk returns a tuple with the SwapIn field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSwapIn

`func (o *BalloonStats) SetSwapIn(v int64)`

SetSwapIn sets SwapIn field to given value.

### HasSwapIn

`func (o *BalloonStats) HasSwapIn() bool`

HasSwapIn returns a boolean if a field has been set.

### GetSwapOut

`func (o *BalloonStats) GetSwapOut() int64`

GetSwapOut returns the SwapOut field if non-nil, zero value otherwise.

### GetSwapOutOk

`func (o *BalloonStats) GetSwapOutOk() (*int64, bool)`

GetSwapOutOk returns a tuple with the SwapOut field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSwapOut

`func (o *BalloonStats) SetSwapOut(v int64)`

SetSwapOut sets SwapOut field to given value.

### HasSwapOut

`func (o *BalloonStats) HasSwapOut() bool`

HasSwapOut returns a boolean if a field has been set.

### GetMajorFaults

`func (o *BalloonStats) GetMajorFaults() int64`

GetMajorFaults returns the MajorFaults field if non-nil, zero value otherwise.

### GetMajorFaultsOk

`func (o *BalloonStats) GetMajorFaultsOk() (*int64, bool)`

GetMajorFaultsOk returns a tuple with the MajorFaults field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMajorFaults

`func (o *BalloonStats) SetMajorFaults(v int64)`

SetMajorFaults sets MajorFaults field to given value.

### HasMajorFaults

`func (o *BalloonStats) HasMajorFaults() bool`

HasMajorFaults returns a boolean if a field has been set.

### GetMinorFaults

`func (o *BalloonStats) GetMinorFaults() int64`

GetMinorFaults returns the MinorFaults field if non-nil, zero value otherwise.

### GetMinorFaultsOk

`func (o *BalloonStats) GetMinorFaultsOk() (*int64, bool)`

GetMinorFaultsOk returns a tuple with the MinorFaults field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMinorFaults

`func (o *BalloonStats) SetMinorFaults(v int64)`

SetMinorFaults sets MinorFaults field to given value.

### HasMinorFaults

`func (o *BalloonStats) HasMinorFaults() bool`

HasMinorFaults returns a boolean if a field has been set.

### GetFreeMemory

`func (o *BalloonStats) GetFreeMemory() int64`

GetFreeMemory returns the FreeMemory field if non-nil, zero value otherwise.

### GetFreeMemoryOk

`func (o *BalloonStats) GetFreeMemoryOk() (*int64, bool)`

GetFreeMemoryOk returns a tuple with the FreeMemory field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFreeMemory

`func (o *BalloonStats) SetFreeMemory(v int64)`

SetFreeMemory sets FreeMemory field to given value.

### HasFreeMemory

`func (o *BalloonStats) HasFreeMemory() bool`

HasFreeMemory returns a boolean if a field has been set.

### GetTotalMemory

`func (o *BalloonStats) GetTotalMemory() int64`

GetTotalMemory returns the TotalMemory field if non-nil, zero value otherwise.

### GetTotalMemoryOk

`func (o *BalloonStats) GetTotalMemoryOk() (*int64, bool)`

GetTotalMemoryOk returns a tuple with the TotalMemory field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTotalMemory

`func (o *BalloonStats) SetTotalMemory(v int64)`

SetTotalMemory sets TotalMemory field to given value.

### HasTotalMemory

`func (o *BalloonStats) HasTotalMemory() bool`

HasTotalMemory returns a boolean if a field has been set.

### GetAvailableMemory

`func (o *BalloonStats) GetAvailableMemory() int64`

GetAvailableMemory returns the AvailableMemory field if non-nil, zero value otherwise.

### GetAvailableMemoryOk

`func (o *BalloonStats) GetAvailableMemoryOk() (*int64, bool)`

GetAvailableMemoryOk returns a tuple with the AvailableMemory field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetAvailableMemory

`func (o *BalloonStats) SetAvailableMemory(v int64)`

SetAvailableMemory sets AvailableMemory field to given value.

### HasAvailableMemory

`func (o *BalloonStats) HasAvailableMemory() bool`

HasAvailableMemory returns a boolean if a field has been set.

### GetDiskCaches

`func (o *BalloonStats) GetDiskCaches() int64`

GetDiskCaches returns the DiskCaches field if non-nil, zero value otherwise.

### GetDiskCachesOk

`func (o *BalloonStats) GetDiskCachesOk() (*int64, bool)`

GetDiskCachesOk returns a tuple with the DiskCaches field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDiskCaches

`func (o *BalloonStats) SetDiskCaches(v int64)`

SetDiskCaches sets DiskCaches field to given value.

### HasDiskCaches

`func (o *BalloonStats) HasDiskCaches() bool`

HasDiskCaches returns a boolean if a field has been set.

### GetHugetlbAllocations

`func (o *BalloonStats) GetHugetlbAllocations() int64`

GetHugetlbAllocations returns the HugetlbAllocations field if non-nil, zero value otherwise.

### GetHugetlbAllocationsOk

`func (o *BalloonStats) GetHugetlbAllocationsOk() (*int64, bool)`

GetHugetlbAllocationsOk returns a tuple with the HugetlbAllocations field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHugetlbAllocations

`func (o *BalloonStats) SetHugetlbAllocations(v int64)`

SetHugetlbAllocations sets HugetlbAllocations field to given value.

### HasHugetlbAllocations

`func (o *BalloonStats) HasHugetlbAllocations() bool`

HasHugetlbAllocations returns a boolean if a field has been set.

### GetHugetlbFailures

`func (o *BalloonStats) GetHugetlbFailures() int64`

GetHugetlbFailures returns the HugetlbFailures field if non-nil, zero value otherwise.

### GetHugetlbFailuresOk

`func (o *BalloonStats) GetHugetlbFailuresOk() (*int64, bool)`

GetHugetlbFailuresOk returns a tuple with the HugetlbFailures field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetHugetlbFailures

`func (o *BalloonStats) SetHugetlbFailures(v int64)`

SetHugetlbFailures sets HugetlbFailures field to given value.

### HasHugetlbFailures

`func (o *BalloonStats) HasHugetlbFailures() bool`

HasHugetlbFailures returns a boolean if a field has been set.

### GetOomKills

`func (o *BalloonStats) GetOomKills() int64`

GetOomKills returns the OomKills field if non-nil, zero value otherwise.

### GetOomKillsOk

`func (o *BalloonStats) GetOomKillsOk() (*int64, bool)`

GetOomKillsOk returns a tuple with the OomKills field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetOomKills

`func (o *BalloonStats) SetOomKills(v int64)`

SetOomKills sets OomKills field to given value.

### HasOomKills

`func (o *BalloonStats) HasOomKills() bool`

HasOomKills returns a boolean if a field has been set.

### GetAllocStalls

`func (o *BalloonStats) GetAllocStalls() int64`

GetAllocStalls returns the AllocStalls field if non-nil, zero value otherwise.

### GetAllocStallsOk

`func (o *BalloonStats) GetAllocStallsOk() (*int64, bool)`

GetAllocStallsOk returns a tuple with the AllocStalls field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetAllocStalls

`func (o *BalloonStats) SetAllocStalls(v int64)`

SetAllocStalls sets AllocStalls field to given value.

### HasAllocStalls

`func (o *BalloonStats) HasAllocStalls() bool`

HasAllocStalls returns a boolean if a field has been set.

### GetAsyncScans

`func (o *BalloonStats) GetAsyncScans() int64`

GetAsyncScans returns the AsyncScans field if non-nil, zero value otherwise.

### GetAsyncScansOk

`func (o *BalloonStats) GetAsyncScansOk() (*int64, bool)`

GetAsyncScansOk returns a tuple with the AsyncScans field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetAsyncScans

`func (o *BalloonStats) SetAsyncScans(v int64)`

SetAsyncScans sets AsyncScans field to given value.

### HasAsyncScans

`func (o *BalloonStats) HasAsyncScans() bool`

HasAsyncScans returns a boolean if a field has been set.

### GetDirectScans

`func (o *BalloonStats) GetDirectScans() int64`

GetDirectScans returns the DirectScans field if non-nil, zero value otherwise.

### GetDirectScansOk

`func (o *BalloonStats) GetDirectScansOk() (*int64, bool)`

GetDirectScansOk returns a tuple with the DirectScans field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDirectScans

`func (o *BalloonStats) SetDirectScans(v int64)`

SetDirectScans sets DirectScans field to given value.

### HasDirectScans

`func (o *BalloonStats) HasDirectScans() bool`

HasDirectScans returns a boolean if a field has been set.

### GetAsyncReclaims

`func (o *BalloonStats) GetAsyncReclaims() int64`

GetAsyncReclaims returns the AsyncReclaims field if non-nil, zero value otherwise.

### GetAsyncReclaimsOk

`func (o *BalloonStats) GetAsyncReclaimsOk() (*int64, bool)`

GetAsyncReclaimsOk returns a tuple with the AsyncReclaims field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetAsyncReclaims

`func (o *BalloonStats) SetAsyncReclaims(v int64)`

SetAsyncReclaims sets AsyncReclaims field to given value.

### HasAsyncReclaims

`func (o *BalloonStats) HasAsyncReclaims() bool`

HasAsyncReclaims returns a boolean if a field has been set.

### GetDirectReclaims

`func (o *BalloonStats) GetDirectReclaims() int64`

GetDirectReclaims returns the DirectReclaims field if non-nil, zero value otherwise.

### GetDirectReclaimsOk

`func (o *BalloonStats) GetDirectReclaimsOk() (*int64, bool)`

GetDirectReclaimsOk returns a tuple with the DirectReclaims field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDirectReclaims

`func (o *BalloonStats) SetDirectReclaims(v int64)`

SetDirectReclaims sets DirectReclaims field to given value.

### HasDirectReclaims

`func (o *BalloonStats) HasDirectReclaims() bool`

HasDirectReclaims returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
