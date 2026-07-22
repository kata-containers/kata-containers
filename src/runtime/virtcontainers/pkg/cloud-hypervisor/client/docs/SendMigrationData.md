# SendMigrationData

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**DestinationUrl** | **string** |  |
**Local** | Pointer to **bool** |  | [optional]
**DowntimeMs** | Pointer to **int64** | The maximum downtime the migration aims for, in milliseconds. Defaults to 300ms.  | [optional] [default to 300]
**TimeoutS** | Pointer to **int64** | The timeout for the migration (maximum total duration), in seconds. Defaults to 3600s (one hour).  | [optional] [default to 3600]
**TimeoutStrategy** | Pointer to [**TimeoutStrategy**](TimeoutStrategy.md) |  | [optional] [default to CANCEL]
**Connections** | Pointer to **int64** | The number of parallel TCP connections to use for migration. Must be between 1 and 128. Multiple connections are not supported with local UNIX-socket migration.  | [optional] [default to 1]
**TlsDir** | Pointer to **string** | Directory containing the TLS root CA certificate (ca-cert.pem), the TLS client certificate (client-cert.pem), and TLS client key (client-key.pem). TLS is only supported with tcp:&lt;host&gt;:&lt;port&gt; destination URLs.  | [optional]
**MemoryMode** | Pointer to [**MigrationMode**](MigrationMode.md) |  | [optional] [default to PRECOPY]

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

### GetDowntimeMs

`func (o *SendMigrationData) GetDowntimeMs() int64`

GetDowntimeMs returns the DowntimeMs field if non-nil, zero value otherwise.

### GetDowntimeMsOk

`func (o *SendMigrationData) GetDowntimeMsOk() (*int64, bool)`

GetDowntimeMsOk returns a tuple with the DowntimeMs field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDowntimeMs

`func (o *SendMigrationData) SetDowntimeMs(v int64)`

SetDowntimeMs sets DowntimeMs field to given value.

### HasDowntimeMs

`func (o *SendMigrationData) HasDowntimeMs() bool`

HasDowntimeMs returns a boolean if a field has been set.

### GetTimeoutS

`func (o *SendMigrationData) GetTimeoutS() int64`

GetTimeoutS returns the TimeoutS field if non-nil, zero value otherwise.

### GetTimeoutSOk

`func (o *SendMigrationData) GetTimeoutSOk() (*int64, bool)`

GetTimeoutSOk returns a tuple with the TimeoutS field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTimeoutS

`func (o *SendMigrationData) SetTimeoutS(v int64)`

SetTimeoutS sets TimeoutS field to given value.

### HasTimeoutS

`func (o *SendMigrationData) HasTimeoutS() bool`

HasTimeoutS returns a boolean if a field has been set.

### GetTimeoutStrategy

`func (o *SendMigrationData) GetTimeoutStrategy() TimeoutStrategy`

GetTimeoutStrategy returns the TimeoutStrategy field if non-nil, zero value otherwise.

### GetTimeoutStrategyOk

`func (o *SendMigrationData) GetTimeoutStrategyOk() (*TimeoutStrategy, bool)`

GetTimeoutStrategyOk returns a tuple with the TimeoutStrategy field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTimeoutStrategy

`func (o *SendMigrationData) SetTimeoutStrategy(v TimeoutStrategy)`

SetTimeoutStrategy sets TimeoutStrategy field to given value.

### HasTimeoutStrategy

`func (o *SendMigrationData) HasTimeoutStrategy() bool`

HasTimeoutStrategy returns a boolean if a field has been set.

### GetConnections

`func (o *SendMigrationData) GetConnections() int64`

GetConnections returns the Connections field if non-nil, zero value otherwise.

### GetConnectionsOk

`func (o *SendMigrationData) GetConnectionsOk() (*int64, bool)`

GetConnectionsOk returns a tuple with the Connections field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetConnections

`func (o *SendMigrationData) SetConnections(v int64)`

SetConnections sets Connections field to given value.

### HasConnections

`func (o *SendMigrationData) HasConnections() bool`

HasConnections returns a boolean if a field has been set.

### GetTlsDir

`func (o *SendMigrationData) GetTlsDir() string`

GetTlsDir returns the TlsDir field if non-nil, zero value otherwise.

### GetTlsDirOk

`func (o *SendMigrationData) GetTlsDirOk() (*string, bool)`

GetTlsDirOk returns a tuple with the TlsDir field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTlsDir

`func (o *SendMigrationData) SetTlsDir(v string)`

SetTlsDir sets TlsDir field to given value.

### HasTlsDir

`func (o *SendMigrationData) HasTlsDir() bool`

HasTlsDir returns a boolean if a field has been set.

### GetMemoryMode

`func (o *SendMigrationData) GetMemoryMode() MigrationMode`

GetMemoryMode returns the MemoryMode field if non-nil, zero value otherwise.

### GetMemoryModeOk

`func (o *SendMigrationData) GetMemoryModeOk() (*MigrationMode, bool)`

GetMemoryModeOk returns a tuple with the MemoryMode field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMemoryMode

`func (o *SendMigrationData) SetMemoryMode(v MigrationMode)`

SetMemoryMode sets MemoryMode field to given value.

### HasMemoryMode

`func (o *SendMigrationData) HasMemoryMode() bool`

HasMemoryMode returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
