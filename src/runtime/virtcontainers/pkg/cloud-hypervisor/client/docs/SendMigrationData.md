# SendMigrationData

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**DestinationUrl** | **string** |  | 
**Local** | Pointer to **bool** |  | [optional] 

## Methods

### NewSendMigrationData

`func NewSendMigrationData(destinationUrl string, ) *SendMigrationData`

NewSendMigrationData instantiates a new SendMigrationData object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewSendMigrationDataWithDefaults

`func NewSendMigrationDataWithDefaults() *SendMigrationData`

NewSendMigrationDataWithDefaults instantiates a new SendMigrationData object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetDestinationUrl

`func (o *SendMigrationData) GetDestinationUrl() string`

GetDestinationUrl returns the DestinationUrl field if non-nil, zero value otherwise.

### GetDestinationUrlOk

`func (o *SendMigrationData) GetDestinationUrlOk() (*string, bool)`

GetDestinationUrlOk returns a tuple with the DestinationUrl field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDestinationUrl

`func (o *SendMigrationData) SetDestinationUrl(v string)`

SetDestinationUrl sets DestinationUrl field to given value.


### GetLocal

`func (o *SendMigrationData) GetLocal() bool`

GetLocal returns the Local field if non-nil, zero value otherwise.

### GetLocalOk

`func (o *SendMigrationData) GetLocalOk() (*bool, bool)`

GetLocalOk returns a tuple with the Local field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetLocal

`func (o *SendMigrationData) SetLocal(v bool)`

SetLocal sets Local field to given value.

### HasLocal

`func (o *SendMigrationData) HasLocal() bool`

HasLocal returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


