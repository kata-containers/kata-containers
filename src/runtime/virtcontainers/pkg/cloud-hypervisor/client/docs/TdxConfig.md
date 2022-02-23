# TdxConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Firmware** | **string** | Path to the firmware that will be used to boot the TDx guest up. | 

## Methods

### NewTdxConfig

`func NewTdxConfig(firmware string, ) *TdxConfig`

NewTdxConfig instantiates a new TdxConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewTdxConfigWithDefaults

`func NewTdxConfigWithDefaults() *TdxConfig`

NewTdxConfigWithDefaults instantiates a new TdxConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetFirmware

`func (o *TdxConfig) GetFirmware() string`

GetFirmware returns the Firmware field if non-nil, zero value otherwise.

### GetFirmwareOk

`func (o *TdxConfig) GetFirmwareOk() (*string, bool)`

GetFirmwareOk returns a tuple with the Firmware field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFirmware

`func (o *TdxConfig) SetFirmware(v string)`

SetFirmware sets Firmware field to given value.



[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


