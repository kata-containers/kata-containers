# SerialConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**File** | Pointer to **string** |  | [optional]
**Socket** | Pointer to **string** |  | [optional]
**Mode** | [**ConsoleMode**](ConsoleMode.md) |  |

## Methods

### NewSerialConfig

`func NewSerialConfig(mode ConsoleMode, ) *SerialConfig`

NewSerialConfig instantiates a new SerialConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewSerialConfigWithDefaults

`func NewSerialConfigWithDefaults() *SerialConfig`

NewSerialConfigWithDefaults instantiates a new SerialConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetFile

`func (o *SerialConfig) GetFile() string`

GetFile returns the File field if non-nil, zero value otherwise.

### GetFileOk

`func (o *SerialConfig) GetFileOk() (*string, bool)`

GetFileOk returns a tuple with the File field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFile

`func (o *SerialConfig) SetFile(v string)`

SetFile sets File field to given value.

### HasFile

`func (o *SerialConfig) HasFile() bool`

HasFile returns a boolean if a field has been set.

### GetSocket

`func (o *SerialConfig) GetSocket() string`

GetSocket returns the Socket field if non-nil, zero value otherwise.

### GetSocketOk

`func (o *SerialConfig) GetSocketOk() (*string, bool)`

GetSocketOk returns a tuple with the Socket field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSocket

`func (o *SerialConfig) SetSocket(v string)`

SetSocket sets Socket field to given value.

### HasSocket

`func (o *SerialConfig) HasSocket() bool`

HasSocket returns a boolean if a field has been set.

### GetMode

`func (o *SerialConfig) GetMode() ConsoleMode`

GetMode returns the Mode field if non-nil, zero value otherwise.

### GetModeOk

`func (o *SerialConfig) GetModeOk() (*ConsoleMode, bool)`

GetModeOk returns a tuple with the Mode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMode

`func (o *SerialConfig) SetMode(v ConsoleMode)`

SetMode sets Mode field to given value.



[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
