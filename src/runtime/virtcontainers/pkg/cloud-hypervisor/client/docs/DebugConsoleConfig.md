# DebugConsoleConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**File** | Pointer to **string** |  | [optional] 
**Mode** | **string** |  | 
**Iobase** | Pointer to **int32** |  | [optional] 

## Methods

### NewDebugConsoleConfig

`func NewDebugConsoleConfig(mode string, ) *DebugConsoleConfig`

NewDebugConsoleConfig instantiates a new DebugConsoleConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewDebugConsoleConfigWithDefaults

`func NewDebugConsoleConfigWithDefaults() *DebugConsoleConfig`

NewDebugConsoleConfigWithDefaults instantiates a new DebugConsoleConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetFile

`func (o *DebugConsoleConfig) GetFile() string`

GetFile returns the File field if non-nil, zero value otherwise.

### GetFileOk

`func (o *DebugConsoleConfig) GetFileOk() (*string, bool)`

GetFileOk returns a tuple with the File field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFile

`func (o *DebugConsoleConfig) SetFile(v string)`

SetFile sets File field to given value.

### HasFile

`func (o *DebugConsoleConfig) HasFile() bool`

HasFile returns a boolean if a field has been set.

### GetMode

`func (o *DebugConsoleConfig) GetMode() string`

GetMode returns the Mode field if non-nil, zero value otherwise.

### GetModeOk

`func (o *DebugConsoleConfig) GetModeOk() (*string, bool)`

GetModeOk returns a tuple with the Mode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMode

`func (o *DebugConsoleConfig) SetMode(v string)`

SetMode sets Mode field to given value.


### GetIobase

`func (o *DebugConsoleConfig) GetIobase() int32`

GetIobase returns the Iobase field if non-nil, zero value otherwise.

### GetIobaseOk

`func (o *DebugConsoleConfig) GetIobaseOk() (*int32, bool)`

GetIobaseOk returns a tuple with the Iobase field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIobase

`func (o *DebugConsoleConfig) SetIobase(v int32)`

SetIobase sets Iobase field to given value.

### HasIobase

`func (o *DebugConsoleConfig) HasIobase() bool`

HasIobase returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


