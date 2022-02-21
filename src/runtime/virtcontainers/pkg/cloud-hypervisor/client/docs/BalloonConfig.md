# BalloonConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Size** | **int64** |  | 
**DeflateOnOom** | Pointer to **bool** | Deflate balloon when the guest is under memory pressure. | [optional] [default to false]
**FreePageReporting** | Pointer to **bool** | Enable guest to report free pages. | [optional] [default to false]

## Methods

### NewBalloonConfig

`func NewBalloonConfig(size int64, ) *BalloonConfig`

NewBalloonConfig instantiates a new BalloonConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewBalloonConfigWithDefaults

`func NewBalloonConfigWithDefaults() *BalloonConfig`

NewBalloonConfigWithDefaults instantiates a new BalloonConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetSize

`func (o *BalloonConfig) GetSize() int64`

GetSize returns the Size field if non-nil, zero value otherwise.

### GetSizeOk

`func (o *BalloonConfig) GetSizeOk() (*int64, bool)`

GetSizeOk returns a tuple with the Size field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSize

`func (o *BalloonConfig) SetSize(v int64)`

SetSize sets Size field to given value.


### GetDeflateOnOom

`func (o *BalloonConfig) GetDeflateOnOom() bool`

GetDeflateOnOom returns the DeflateOnOom field if non-nil, zero value otherwise.

### GetDeflateOnOomOk

`func (o *BalloonConfig) GetDeflateOnOomOk() (*bool, bool)`

GetDeflateOnOomOk returns a tuple with the DeflateOnOom field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDeflateOnOom

`func (o *BalloonConfig) SetDeflateOnOom(v bool)`

SetDeflateOnOom sets DeflateOnOom field to given value.

### HasDeflateOnOom

`func (o *BalloonConfig) HasDeflateOnOom() bool`

HasDeflateOnOom returns a boolean if a field has been set.

### GetFreePageReporting

`func (o *BalloonConfig) GetFreePageReporting() bool`

GetFreePageReporting returns the FreePageReporting field if non-nil, zero value otherwise.

### GetFreePageReportingOk

`func (o *BalloonConfig) GetFreePageReportingOk() (*bool, bool)`

GetFreePageReportingOk returns a tuple with the FreePageReporting field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFreePageReporting

`func (o *BalloonConfig) SetFreePageReporting(v bool)`

SetFreePageReporting sets FreePageReporting field to given value.

### HasFreePageReporting

`func (o *BalloonConfig) HasFreePageReporting() bool`

HasFreePageReporting returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


