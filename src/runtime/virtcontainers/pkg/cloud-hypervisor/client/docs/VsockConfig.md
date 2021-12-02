# VsockConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Cid** | **int64** | Guest Vsock CID | 
**Socket** | **string** | Path to UNIX domain socket, used to proxy vsock connections. | 
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**PciSegment** | Pointer to **int32** |  | [optional] 
**Id** | Pointer to **string** |  | [optional] 

## Methods

### NewVsockConfig

`func NewVsockConfig(cid int64, socket string, ) *VsockConfig`

NewVsockConfig instantiates a new VsockConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVsockConfigWithDefaults

`func NewVsockConfigWithDefaults() *VsockConfig`

NewVsockConfigWithDefaults instantiates a new VsockConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetCid

`func (o *VsockConfig) GetCid() int64`

GetCid returns the Cid field if non-nil, zero value otherwise.

### GetCidOk

`func (o *VsockConfig) GetCidOk() (*int64, bool)`

GetCidOk returns a tuple with the Cid field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetCid

`func (o *VsockConfig) SetCid(v int64)`

SetCid sets Cid field to given value.


### GetSocket

`func (o *VsockConfig) GetSocket() string`

GetSocket returns the Socket field if non-nil, zero value otherwise.

### GetSocketOk

`func (o *VsockConfig) GetSocketOk() (*string, bool)`

GetSocketOk returns a tuple with the Socket field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSocket

`func (o *VsockConfig) SetSocket(v string)`

SetSocket sets Socket field to given value.


### GetIommu

`func (o *VsockConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *VsockConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *VsockConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *VsockConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.

### GetPciSegment

`func (o *VsockConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *VsockConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *VsockConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *VsockConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetId

`func (o *VsockConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *VsockConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *VsockConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *VsockConfig) HasId() bool`

HasId returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


