# VmResizeDisk

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | Pointer to **string** | disk identifier | [optional] 
**DesiredSize** | Pointer to **int64** | desired disk size in bytes | [optional] 

## Methods

### NewVmResizeDisk

`func NewVmResizeDisk() *VmResizeDisk`

NewVmResizeDisk instantiates a new VmResizeDisk object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVmResizeDiskWithDefaults

`func NewVmResizeDiskWithDefaults() *VmResizeDisk`

NewVmResizeDiskWithDefaults instantiates a new VmResizeDisk object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *VmResizeDisk) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *VmResizeDisk) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *VmResizeDisk) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *VmResizeDisk) HasId() bool`

HasId returns a boolean if a field has been set.

### GetDesiredSize

`func (o *VmResizeDisk) GetDesiredSize() int64`

GetDesiredSize returns the DesiredSize field if non-nil, zero value otherwise.

### GetDesiredSizeOk

`func (o *VmResizeDisk) GetDesiredSizeOk() (*int64, bool)`

GetDesiredSizeOk returns a tuple with the DesiredSize field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDesiredSize

`func (o *VmResizeDisk) SetDesiredSize(v int64)`

SetDesiredSize sets DesiredSize field to given value.

### HasDesiredSize

`func (o *VmResizeDisk) HasDesiredSize() bool`

HasDesiredSize returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


