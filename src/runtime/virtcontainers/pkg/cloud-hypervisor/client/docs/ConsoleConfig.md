# ConsoleConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**File** | Pointer to **string** |  | [optional]
**Socket** | Pointer to **string** |  | [optional]
**Mode** | [**ConsoleMode**](ConsoleMode.md) |  |
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**Id** | Pointer to **string** |  | [optional]
**PciSegment** | Pointer to **int32** |  | [optional]
**PciDeviceId** | Pointer to **int32** |  | [optional]

## Methods

### NewConsoleConfig

`func NewConsoleConfig(mode ConsoleMode, ) *ConsoleConfig`

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

`func (o *ConsoleConfig) GetMode() ConsoleMode`

GetMode returns the Mode field if non-nil, zero value otherwise.

### GetModeOk

`func (o *ConsoleConfig) GetModeOk() (*ConsoleMode, bool)`

GetModeOk returns a tuple with the Mode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMode

`func (o *ConsoleConfig) SetMode(v ConsoleMode)`

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

### GetId

`func (o *ConsoleConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *ConsoleConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *ConsoleConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *ConsoleConfig) HasId() bool`

HasId returns a boolean if a field has been set.

### GetPciSegment

`func (o *ConsoleConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *ConsoleConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *ConsoleConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *ConsoleConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetPciDeviceId

`func (o *ConsoleConfig) GetPciDeviceId() int32`

GetPciDeviceId returns the PciDeviceId field if non-nil, zero value otherwise.

### GetPciDeviceIdOk

`func (o *ConsoleConfig) GetPciDeviceIdOk() (*int32, bool)`

GetPciDeviceIdOk returns a tuple with the PciDeviceId field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciDeviceId

`func (o *ConsoleConfig) SetPciDeviceId(v int32)`

SetPciDeviceId sets PciDeviceId field to given value.

### HasPciDeviceId

`func (o *ConsoleConfig) HasPciDeviceId() bool`

HasPciDeviceId returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
