# VdpaConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Path** | **string** |  | 
**NumQueues** | **int32** |  | [default to 1]
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**PciSegment** | Pointer to **int32** |  | [optional] 
**Id** | Pointer to **string** |  | [optional] 

## Methods

### NewVdpaConfig

`func NewVdpaConfig(path string, numQueues int32, ) *VdpaConfig`

NewVdpaConfig instantiates a new VdpaConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVdpaConfigWithDefaults

`func NewVdpaConfigWithDefaults() *VdpaConfig`

NewVdpaConfigWithDefaults instantiates a new VdpaConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetPath

`func (o *VdpaConfig) GetPath() string`

GetPath returns the Path field if non-nil, zero value otherwise.

### GetPathOk

`func (o *VdpaConfig) GetPathOk() (*string, bool)`

GetPathOk returns a tuple with the Path field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPath

`func (o *VdpaConfig) SetPath(v string)`

SetPath sets Path field to given value.


### GetNumQueues

`func (o *VdpaConfig) GetNumQueues() int32`

GetNumQueues returns the NumQueues field if non-nil, zero value otherwise.

### GetNumQueuesOk

`func (o *VdpaConfig) GetNumQueuesOk() (*int32, bool)`

GetNumQueuesOk returns a tuple with the NumQueues field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetNumQueues

`func (o *VdpaConfig) SetNumQueues(v int32)`

SetNumQueues sets NumQueues field to given value.


### GetIommu

`func (o *VdpaConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *VdpaConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *VdpaConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *VdpaConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.

### GetPciSegment

`func (o *VdpaConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *VdpaConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *VdpaConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *VdpaConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetId

`func (o *VdpaConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *VdpaConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *VdpaConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *VdpaConfig) HasId() bool`

HasId returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


