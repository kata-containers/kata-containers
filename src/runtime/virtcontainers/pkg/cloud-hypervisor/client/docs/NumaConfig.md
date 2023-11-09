# NumaConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**GuestNumaId** | **int32** |  | 
**Cpus** | Pointer to **[]int32** |  | [optional] 
**Distances** | Pointer to [**[]NumaDistance**](NumaDistance.md) |  | [optional] 
**MemoryZones** | Pointer to **[]string** |  | [optional] 
**SgxEpcSections** | Pointer to **[]string** |  | [optional] 
**PciSegments** | Pointer to **[]int32** |  | [optional] 

## Methods

### NewNumaConfig

`func NewNumaConfig(guestNumaId int32, ) *NumaConfig`

NewNumaConfig instantiates a new NumaConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewNumaConfigWithDefaults

`func NewNumaConfigWithDefaults() *NumaConfig`

NewNumaConfigWithDefaults instantiates a new NumaConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetGuestNumaId

`func (o *NumaConfig) GetGuestNumaId() int32`

GetGuestNumaId returns the GuestNumaId field if non-nil, zero value otherwise.

### GetGuestNumaIdOk

`func (o *NumaConfig) GetGuestNumaIdOk() (*int32, bool)`

GetGuestNumaIdOk returns a tuple with the GuestNumaId field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetGuestNumaId

`func (o *NumaConfig) SetGuestNumaId(v int32)`

SetGuestNumaId sets GuestNumaId field to given value.


### GetCpus

`func (o *NumaConfig) GetCpus() []int32`

GetCpus returns the Cpus field if non-nil, zero value otherwise.

### GetCpusOk

`func (o *NumaConfig) GetCpusOk() (*[]int32, bool)`

GetCpusOk returns a tuple with the Cpus field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetCpus

`func (o *NumaConfig) SetCpus(v []int32)`

SetCpus sets Cpus field to given value.

### HasCpus

`func (o *NumaConfig) HasCpus() bool`

HasCpus returns a boolean if a field has been set.

### GetDistances

`func (o *NumaConfig) GetDistances() []NumaDistance`

GetDistances returns the Distances field if non-nil, zero value otherwise.

### GetDistancesOk

`func (o *NumaConfig) GetDistancesOk() (*[]NumaDistance, bool)`

GetDistancesOk returns a tuple with the Distances field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDistances

`func (o *NumaConfig) SetDistances(v []NumaDistance)`

SetDistances sets Distances field to given value.

### HasDistances

`func (o *NumaConfig) HasDistances() bool`

HasDistances returns a boolean if a field has been set.

### GetMemoryZones

`func (o *NumaConfig) GetMemoryZones() []string`

GetMemoryZones returns the MemoryZones field if non-nil, zero value otherwise.

### GetMemoryZonesOk

`func (o *NumaConfig) GetMemoryZonesOk() (*[]string, bool)`

GetMemoryZonesOk returns a tuple with the MemoryZones field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMemoryZones

`func (o *NumaConfig) SetMemoryZones(v []string)`

SetMemoryZones sets MemoryZones field to given value.

### HasMemoryZones

`func (o *NumaConfig) HasMemoryZones() bool`

HasMemoryZones returns a boolean if a field has been set.

### GetSgxEpcSections

`func (o *NumaConfig) GetSgxEpcSections() []string`

GetSgxEpcSections returns the SgxEpcSections field if non-nil, zero value otherwise.

### GetSgxEpcSectionsOk

`func (o *NumaConfig) GetSgxEpcSectionsOk() (*[]string, bool)`

GetSgxEpcSectionsOk returns a tuple with the SgxEpcSections field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSgxEpcSections

`func (o *NumaConfig) SetSgxEpcSections(v []string)`

SetSgxEpcSections sets SgxEpcSections field to given value.

### HasSgxEpcSections

`func (o *NumaConfig) HasSgxEpcSections() bool`

HasSgxEpcSections returns a boolean if a field has been set.

### GetPciSegments

`func (o *NumaConfig) GetPciSegments() []int32`

GetPciSegments returns the PciSegments field if non-nil, zero value otherwise.

### GetPciSegmentsOk

`func (o *NumaConfig) GetPciSegmentsOk() (*[]int32, bool)`

GetPciSegmentsOk returns a tuple with the PciSegments field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegments

`func (o *NumaConfig) SetPciSegments(v []int32)`

SetPciSegments sets PciSegments field to given value.

### HasPciSegments

`func (o *NumaConfig) HasPciSegments() bool`

HasPciSegments returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


