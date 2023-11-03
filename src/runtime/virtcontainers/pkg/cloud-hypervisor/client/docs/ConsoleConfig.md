# ConsoleConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**File** | Pointer to **string** |  | [optional] 
**Socket** | Pointer to **string** |  | [optional] 
**Mode** | **string** |  | 
**Iommu** | Pointer to **bool** |  | [optional] [default to false]

## Methods

### NewConsoleConfig

`func NewConsoleConfig(mode string, ) *ConsoleConfig`

NewConsoleConfig instantiates a new ConsoleConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewConsoleConfigWithDefaults

`func NewConsoleConfigWithDefaults() *ConsoleConfig`

NewConsoleConfigWithDefaults instantiates a new ConsoleConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetFile

`func (o *ConsoleConfig) GetFile() string`

GetFile returns the File field if non-nil, zero value otherwise.

### GetFileOk

`func (o *ConsoleConfig) GetFileOk() (*string, bool)`

GetFileOk returns a tuple with the File field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFile

`func (o *ConsoleConfig) SetFile(v string)`

SetFile sets File field to given value.

### HasFile

`func (o *ConsoleConfig) HasFile() bool`

HasFile returns a boolean if a field has been set.

### GetSocket

`func (o *ConsoleConfig) GetSocket() string`

GetSocket returns the Socket field if non-nil, zero value otherwise.

### GetSocketOk

`func (o *ConsoleConfig) GetSocketOk() (*string, bool)`

GetSocketOk returns a tuple with the Socket field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSocket

`func (o *ConsoleConfig) SetSocket(v string)`

SetSocket sets Socket field to given value.

### HasSocket

`func (o *ConsoleConfig) HasSocket() bool`

HasSocket returns a boolean if a field has been set.

### GetMode

`func (o *ConsoleConfig) GetMode() string`

GetMode returns the Mode field if non-nil, zero value otherwise.

### GetModeOk

`func (o *ConsoleConfig) GetModeOk() (*string, bool)`

GetModeOk returns a tuple with the Mode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMode

`func (o *ConsoleConfig) SetMode(v string)`

SetMode sets Mode field to given value.


### GetIommu

`func (o *ConsoleConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *ConsoleConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *ConsoleConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *ConsoleConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


