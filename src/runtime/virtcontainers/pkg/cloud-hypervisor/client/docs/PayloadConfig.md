# PayloadConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Firmware** | Pointer to **string** |  | [optional] 
**Kernel** | Pointer to **string** |  | [optional] 
**Cmdline** | Pointer to **string** |  | [optional] 
**Initramfs** | Pointer to **string** |  | [optional] 

## Methods

### NewPayloadConfig

`func NewPayloadConfig() *PayloadConfig`

NewPayloadConfig instantiates a new PayloadConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewPayloadConfigWithDefaults

`func NewPayloadConfigWithDefaults() *PayloadConfig`

NewPayloadConfigWithDefaults instantiates a new PayloadConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetFirmware

`func (o *PayloadConfig) GetFirmware() string`

GetFirmware returns the Firmware field if non-nil, zero value otherwise.

### GetFirmwareOk

`func (o *PayloadConfig) GetFirmwareOk() (*string, bool)`

GetFirmwareOk returns a tuple with the Firmware field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFirmware

`func (o *PayloadConfig) SetFirmware(v string)`

SetFirmware sets Firmware field to given value.

### HasFirmware

`func (o *PayloadConfig) HasFirmware() bool`

HasFirmware returns a boolean if a field has been set.

### GetKernel

`func (o *PayloadConfig) GetKernel() string`

GetKernel returns the Kernel field if non-nil, zero value otherwise.

### GetKernelOk

`func (o *PayloadConfig) GetKernelOk() (*string, bool)`

GetKernelOk returns a tuple with the Kernel field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetKernel

`func (o *PayloadConfig) SetKernel(v string)`

SetKernel sets Kernel field to given value.

### HasKernel

`func (o *PayloadConfig) HasKernel() bool`

HasKernel returns a boolean if a field has been set.

### GetCmdline

`func (o *PayloadConfig) GetCmdline() string`

GetCmdline returns the Cmdline field if non-nil, zero value otherwise.

### GetCmdlineOk

`func (o *PayloadConfig) GetCmdlineOk() (*string, bool)`

GetCmdlineOk returns a tuple with the Cmdline field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetCmdline

`func (o *PayloadConfig) SetCmdline(v string)`

SetCmdline sets Cmdline field to given value.

### HasCmdline

`func (o *PayloadConfig) HasCmdline() bool`

HasCmdline returns a boolean if a field has been set.

### GetInitramfs

`func (o *PayloadConfig) GetInitramfs() string`

GetInitramfs returns the Initramfs field if non-nil, zero value otherwise.

### GetInitramfsOk

`func (o *PayloadConfig) GetInitramfsOk() (*string, bool)`

GetInitramfsOk returns a tuple with the Initramfs field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetInitramfs

`func (o *PayloadConfig) SetInitramfs(v string)`

SetInitramfs sets Initramfs field to given value.

### HasInitramfs

`func (o *PayloadConfig) HasInitramfs() bool`

HasInitramfs returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


