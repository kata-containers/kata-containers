# ReceiveMigrationData

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ReceiverUrl** | **string** |  |
**TlsDir** | Pointer to **string** | Directory containing the TLS server certificate (server-cert.pem), the TLS server key (server-key.pem), and the client TLS root CA certificate (ca-cert.pem). TLS is only supported with tcp:&lt;host&gt;:&lt;port&gt; receiver URLs.  | [optional]
**MemoryMode** | Pointer to [**MigrationMode**](MigrationMode.md) |  | [optional] [default to PRECOPY]

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

### GetMemoryMode

`func (o *ReceiveMigrationData) GetMemoryMode() MigrationMode`

GetMemoryMode returns the MemoryMode field if non-nil, zero value otherwise.

### GetMemoryModeOk

`func (o *ReceiveMigrationData) GetMemoryModeOk() (*MigrationMode, bool)`

GetMemoryModeOk returns a tuple with the MemoryMode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMemoryMode

`func (o *ReceiveMigrationData) SetMemoryMode(v MigrationMode)`

SetMemoryMode sets MemoryMode field to given value.

### HasMemoryMode

`func (o *ReceiveMigrationData) HasMemoryMode() bool`

HasMemoryMode returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
