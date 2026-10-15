# ReceiveMigrationData

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ReceiverUrl** | **string** |  | 
**TlsDir** | Pointer to **string** | Directory containing the TLS server certificate (server-cert.pem), the TLS server key (server-key.pem), and the client TLS root CA certificate (ca-cert.pem). TLS is only supported with tcp:&lt;host&gt;:&lt;port&gt; receiver URLs. Recommended for all TCP migrations. Omitting it is only intended for development and testing, as TCP migration is then unencrypted and unauthenticated.  | [optional]
**ZoneUpdates** | Pointer to [**[]VmMemoryZoneUpdateData**](VmMemoryZoneUpdateData.md) |  | [optional]
**BackgroundFetchPages** | Pointer to **bool** | Enable the background thread for faulting all remaining pages. | [optional] [default to true]

## Methods

### NewReceiveMigrationData

`func NewReceiveMigrationData(receiverUrl string, ) *ReceiveMigrationData`

NewReceiveMigrationData instantiates a new ReceiveMigrationData object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewReceiveMigrationDataWithDefaults

`func NewReceiveMigrationDataWithDefaults() *ReceiveMigrationData`

NewReceiveMigrationDataWithDefaults instantiates a new ReceiveMigrationData object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetReceiverUrl

`func (o *ReceiveMigrationData) GetReceiverUrl() string`

GetReceiverUrl returns the ReceiverUrl field if non-nil, zero value otherwise.

### GetReceiverUrlOk

`func (o *ReceiveMigrationData) GetReceiverUrlOk() (*string, bool)`

GetReceiverUrlOk returns a tuple with the ReceiverUrl field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetReceiverUrl

`func (o *ReceiveMigrationData) SetReceiverUrl(v string)`

SetReceiverUrl sets ReceiverUrl field to given value.


### GetTlsDir

`func (o *ReceiveMigrationData) GetTlsDir() string`

GetTlsDir returns the TlsDir field if non-nil, zero value otherwise.

### GetTlsDirOk

`func (o *ReceiveMigrationData) GetTlsDirOk() (*string, bool)`

GetTlsDirOk returns a tuple with the TlsDir field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTlsDir

`func (o *ReceiveMigrationData) SetTlsDir(v string)`

SetTlsDir sets TlsDir field to given value.

### HasTlsDir

`func (o *ReceiveMigrationData) HasTlsDir() bool`

HasTlsDir returns a boolean if a field has been set.

### GetZoneUpdates

`func (o *ReceiveMigrationData) GetZoneUpdates() []VmMemoryZoneUpdateData`

GetZoneUpdates returns the ZoneUpdates field if non-nil, zero value otherwise.

### GetZoneUpdatesOk

`func (o *ReceiveMigrationData) GetZoneUpdatesOk() (*[]VmMemoryZoneUpdateData, bool)`

GetZoneUpdatesOk returns a tuple with the ZoneUpdates field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetZoneUpdates

`func (o *ReceiveMigrationData) SetZoneUpdates(v []VmMemoryZoneUpdateData)`

SetZoneUpdates sets ZoneUpdates field to given value.

### HasZoneUpdates

`func (o *ReceiveMigrationData) HasZoneUpdates() bool`

HasZoneUpdates returns a boolean if a field has been set.

### GetBackgroundFetchPages

`func (o *ReceiveMigrationData) GetBackgroundFetchPages() bool`

GetBackgroundFetchPages returns the BackgroundFetchPages field if non-nil, zero value otherwise.

### GetBackgroundFetchPagesOk

`func (o *ReceiveMigrationData) GetBackgroundFetchPagesOk() (*bool, bool)`

GetBackgroundFetchPagesOk returns a tuple with the BackgroundFetchPages field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetBackgroundFetchPages

`func (o *ReceiveMigrationData) SetBackgroundFetchPages(v bool)`

SetBackgroundFetchPages sets BackgroundFetchPages field to given value.

### HasBackgroundFetchPages

`func (o *ReceiveMigrationData) HasBackgroundFetchPages() bool`

HasBackgroundFetchPages returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


