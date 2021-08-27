# PciDeviceInfo

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Id** | **string** |  | 
**Bdf** | **string** |  | 

## Methods

### NewPciDeviceInfo

`func NewPciDeviceInfo(id string, bdf string, ) *PciDeviceInfo`

NewPciDeviceInfo instantiates a new PciDeviceInfo object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewPciDeviceInfoWithDefaults

`func NewPciDeviceInfoWithDefaults() *PciDeviceInfo`

NewPciDeviceInfoWithDefaults instantiates a new PciDeviceInfo object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetId

`func (o *PciDeviceInfo) GetId() string`

GetId returns the Id field if non-nil, zero value otherwise.

### GetIdOk

`func (o *PciDeviceInfo) GetIdOk() (*string, bool)`

GetIdOk returns a tuple with the Id field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetId

`func (o *PciDeviceInfo) SetId(v string)`

SetId sets Id field to given value.


### GetBdf

`func (o *PciDeviceInfo) GetBdf() string`

GetBdf returns the Bdf field if non-nil, zero value otherwise.

### GetBdfOk

`func (o *PciDeviceInfo) GetBdfOk() (*string, bool)`

GetBdfOk returns a tuple with the Bdf field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetBdf

`func (o *PciDeviceInfo) SetBdf(v string)`

SetBdf sets Bdf field to given value.



[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


