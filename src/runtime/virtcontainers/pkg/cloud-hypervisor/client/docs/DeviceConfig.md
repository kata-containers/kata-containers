# DeviceConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Path** | **string** |  | 
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**PciSegment** | Pointer to **int32** |  | [optional] 
**Id** | Pointer to **string** |  | [optional] 
**XNvGpudirectClique** | Pointer to **int32** |  | [optional] 

## Methods

### NewDeviceConfig

`func NewDeviceConfig(path string, ) *DeviceConfig`

NewDeviceConfig instantiates a new DeviceConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewDeviceConfigWithDefaults

`func NewDeviceConfigWithDefaults() *DeviceConfig`

NewDeviceConfigWithDefaults instantiates a new DeviceConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetPath

`func (o *DeviceConfig) GetPath() string`

GetPath returns the Path field if non-nil, zero value otherwise.

### GetPathOk

`func (o *DeviceConfig) GetPathOk() (*string, bool)`

GetPathOk returns a tuple with the Path field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPath

`func (o *DeviceConfig) SetPath(v string)`

SetPath sets Path field to given value.


### GetIommu

`func (o *DeviceConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *DeviceConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *DeviceConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *DeviceConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.

### GetPciSegment

`func (o *DeviceConfig) GetPciSegment() int32`

GetPciSegment returns the PciSegment field if non-nil, zero value otherwise.

### GetPciSegmentOk

`func (o *DeviceConfig) GetPciSegmentOk() (*int32, bool)`

GetPciSegmentOk returns a tuple with the PciSegment field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegment

`func (o *DeviceConfig) SetPciSegment(v int32)`

SetPciSegment sets PciSegment field to given value.

### HasPciSegment

`func (o *DeviceConfig) HasPciSegment() bool`

HasPciSegment returns a boolean if a field has been set.

### GetId

`func (o *DeviceConfig) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *DeviceConfig) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *DeviceConfig) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *DeviceConfig) HasId() bool`

HasId returns a boolean if a field has been set.

### GetXNvGpudirectClique

`func (o *DeviceConfig) GetXNvGpudirectClique() int32`

GetXNvGpudirectClique returns the XNvGpudirectClique field if non-nil, zero value otherwise.

### GetXNvGpudirectCliqueOk

`func (o *DeviceConfig) GetXNvGpudirectCliqueOk() (*int32, bool)`

GetXNvGpudirectCliqueOk returns a tuple with the XNvGpudirectClique field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetXNvGpudirectClique

`func (o *DeviceConfig) SetXNvGpudirectClique(v int32)`

SetXNvGpudirectClique sets XNvGpudirectClique field to given value.

### HasXNvGpudirectClique

`func (o *DeviceConfig) HasXNvGpudirectClique() bool`

HasXNvGpudirectClique returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


