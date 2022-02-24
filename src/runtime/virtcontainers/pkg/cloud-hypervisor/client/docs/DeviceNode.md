# DeviceNode

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | Pointer to **string** |  | [optional] 
**Resources** | Pointer to **[]map[string]interface{}** |  | [optional] 
**Children** | Pointer to **[]string** |  | [optional] 
**PciBdf** | Pointer to **string** |  | [optional] 

## Methods

### NewDeviceNode

`func NewDeviceNode() *DeviceNode`

NewDeviceNode instantiates a new DeviceNode object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewDeviceNodeWithDefaults

`func NewDeviceNodeWithDefaults() *DeviceNode`

NewDeviceNodeWithDefaults instantiates a new DeviceNode object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *DeviceNode) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *DeviceNode) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *DeviceNode) SetId(v string)`

SetId sets Id field to given value.

### HasId

`func (o *DeviceNode) HasId() bool`

HasId returns a boolean if a field has been set.

### GetResources

`func (o *DeviceNode) GetResources() []map[string]interface{}`

GetResources returns the Resources field if non-nil, zero value otherwise.

### GetResourcesOk

`func (o *DeviceNode) GetResourcesOk() (*[]map[string]interface{}, bool)`

GetResourcesOk returns a tuple with the Resources field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetResources

`func (o *DeviceNode) SetResources(v []map[string]interface{})`

SetResources sets Resources field to given value.

### HasResources

`func (o *DeviceNode) HasResources() bool`

HasResources returns a boolean if a field has been set.

### GetChildren

`func (o *DeviceNode) GetChildren() []string`

GetChildren returns the Children field if non-nil, zero value otherwise.

### GetChildrenOk

`func (o *DeviceNode) GetChildrenOk() (*[]string, bool)`

GetChildrenOk returns a tuple with the Children field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetChildren

`func (o *DeviceNode) SetChildren(v []string)`

SetChildren sets Children field to given value.

### HasChildren

`func (o *DeviceNode) HasChildren() bool`

HasChildren returns a boolean if a field has been set.

### GetPciBdf

`func (o *DeviceNode) GetPciBdf() string`

GetPciBdf returns the PciBdf field if non-nil, zero value otherwise.

### GetPciBdfOk

`func (o *DeviceNode) GetPciBdfOk() (*string, bool)`

GetPciBdfOk returns a tuple with the PciBdf field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciBdf

`func (o *DeviceNode) SetPciBdf(v string)`

SetPciBdf sets PciBdf field to given value.

### HasPciBdf

`func (o *DeviceNode) HasPciBdf() bool`

HasPciBdf returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


