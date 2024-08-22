# VmConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**Cpus** | Pointer to [**CpusConfig**](CpusConfig.md) |  | [optional] 
**Memory** | Pointer to [**MemoryConfig**](MemoryConfig.md) |  | [optional] 
**Payload** | [**PayloadConfig**](PayloadConfig.md) |  | 
**RateLimitGroups** | Pointer to [**[]RateLimitGroupConfig**](RateLimitGroupConfig.md) |  | [optional] 
**Disks** | Pointer to [**[]DiskConfig**](DiskConfig.md) |  | [optional] 
**Net** | Pointer to [**[]NetConfig**](NetConfig.md) |  | [optional] 
**Rng** | Pointer to [**RngConfig**](RngConfig.md) |  | [optional] 
**Balloon** | Pointer to [**BalloonConfig**](BalloonConfig.md) |  | [optional] 
**Fs** | Pointer to [**[]FsConfig**](FsConfig.md) |  | [optional] 
**Pmem** | Pointer to [**[]PmemConfig**](PmemConfig.md) |  | [optional] 
**Serial** | Pointer to [**ConsoleConfig**](ConsoleConfig.md) |  | [optional] 
**Console** | Pointer to [**ConsoleConfig**](ConsoleConfig.md) |  | [optional] 
**DebugConsole** | Pointer to [**DebugConsoleConfig**](DebugConsoleConfig.md) |  | [optional] 
**Devices** | Pointer to [**[]DeviceConfig**](DeviceConfig.md) |  | [optional] 
**Vdpa** | Pointer to [**[]VdpaConfig**](VdpaConfig.md) |  | [optional] 
**Vsock** | Pointer to [**VsockConfig**](VsockConfig.md) |  | [optional] 
**SgxEpc** | Pointer to [**[]SgxEpcConfig**](SgxEpcConfig.md) |  | [optional] 
**Numa** | Pointer to [**[]NumaConfig**](NumaConfig.md) |  | [optional] 
**Iommu** | Pointer to **bool** |  | [optional] [default to false]
**Watchdog** | Pointer to **bool** |  | [optional] [default to false]
**Pvpanic** | Pointer to **bool** |  | [optional] [default to false]
**PciSegments** | Pointer to [**[]PciSegmentConfig**](PciSegmentConfig.md) |  | [optional] 
**Platform** | Pointer to [**PlatformConfig**](PlatformConfig.md) |  | [optional] 
**Tpm** | Pointer to [**TpmConfig**](TpmConfig.md) |  | [optional] 
**LandlockEnable** | Pointer to **bool** |  | [optional] [default to false]
**LandlockRules** | Pointer to [**[]LandlockConfig**](LandlockConfig.md) |  | [optional] 

## Methods

### NewVmConfig

`func NewVmConfig(payload PayloadConfig, ) *VmConfig`

NewVmConfig instantiates a new VmConfig object
This constructor will assign default values to properties that have it defined,
and makes sure properties required by API are set, but the set of arguments
will change when the set of required properties is changed

### NewVmConfigWithDefaults

`func NewVmConfigWithDefaults() *VmConfig`

NewVmConfigWithDefaults instantiates a new VmConfig object
This constructor will only assign default values to properties that have it defined,
but it doesn't guarantee that properties required by API are set

### GetCpus

`func (o *VmConfig) GetCpus() CpusConfig`

GetCpus returns the Cpus field if non-nil, zero value otherwise.

### GetCpusOk

`func (o *VmConfig) GetCpusOk() (*CpusConfig, bool)`

GetCpusOk returns a tuple with the Cpus field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetCpus

`func (o *VmConfig) SetCpus(v CpusConfig)`

SetCpus sets Cpus field to given value.

### HasCpus

`func (o *VmConfig) HasCpus() bool`

HasCpus returns a boolean if a field has been set.

### GetMemory

`func (o *VmConfig) GetMemory() MemoryConfig`

GetMemory returns the Memory field if non-nil, zero value otherwise.

### GetMemoryOk

`func (o *VmConfig) GetMemoryOk() (*MemoryConfig, bool)`

GetMemoryOk returns a tuple with the Memory field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetMemory

`func (o *VmConfig) SetMemory(v MemoryConfig)`

SetMemory sets Memory field to given value.

### HasMemory

`func (o *VmConfig) HasMemory() bool`

HasMemory returns a boolean if a field has been set.

### GetPayload

`func (o *VmConfig) GetPayload() PayloadConfig`

GetPayload returns the Payload field if non-nil, zero value otherwise.

### GetPayloadOk

`func (o *VmConfig) GetPayloadOk() (*PayloadConfig, bool)`

GetPayloadOk returns a tuple with the Payload field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPayload

`func (o *VmConfig) SetPayload(v PayloadConfig)`

SetPayload sets Payload field to given value.


### GetRateLimitGroups

`func (o *VmConfig) GetRateLimitGroups() []RateLimitGroupConfig`

GetRateLimitGroups returns the RateLimitGroups field if non-nil, zero value otherwise.

### GetRateLimitGroupsOk

`func (o *VmConfig) GetRateLimitGroupsOk() (*[]RateLimitGroupConfig, bool)`

GetRateLimitGroupsOk returns a tuple with the RateLimitGroups field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetRateLimitGroups

`func (o *VmConfig) SetRateLimitGroups(v []RateLimitGroupConfig)`

SetRateLimitGroups sets RateLimitGroups field to given value.

### HasRateLimitGroups

`func (o *VmConfig) HasRateLimitGroups() bool`

HasRateLimitGroups returns a boolean if a field has been set.

### GetDisks

`func (o *VmConfig) GetDisks() []DiskConfig`

GetDisks returns the Disks field if non-nil, zero value otherwise.

### GetDisksOk

`func (o *VmConfig) GetDisksOk() (*[]DiskConfig, bool)`

GetDisksOk returns a tuple with the Disks field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDisks

`func (o *VmConfig) SetDisks(v []DiskConfig)`

SetDisks sets Disks field to given value.

### HasDisks

`func (o *VmConfig) HasDisks() bool`

HasDisks returns a boolean if a field has been set.

### GetNet

`func (o *VmConfig) GetNet() []NetConfig`

GetNet returns the Net field if non-nil, zero value otherwise.

### GetNetOk

`func (o *VmConfig) GetNetOk() (*[]NetConfig, bool)`

GetNetOk returns a tuple with the Net field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetNet

`func (o *VmConfig) SetNet(v []NetConfig)`

SetNet sets Net field to given value.

### HasNet

`func (o *VmConfig) HasNet() bool`

HasNet returns a boolean if a field has been set.

### GetRng

`func (o *VmConfig) GetRng() RngConfig`

GetRng returns the Rng field if non-nil, zero value otherwise.

### GetRngOk

`func (o *VmConfig) GetRngOk() (*RngConfig, bool)`

GetRngOk returns a tuple with the Rng field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetRng

`func (o *VmConfig) SetRng(v RngConfig)`

SetRng sets Rng field to given value.

### HasRng

`func (o *VmConfig) HasRng() bool`

HasRng returns a boolean if a field has been set.

### GetBalloon

`func (o *VmConfig) GetBalloon() BalloonConfig`

GetBalloon returns the Balloon field if non-nil, zero value otherwise.

### GetBalloonOk

`func (o *VmConfig) GetBalloonOk() (*BalloonConfig, bool)`

GetBalloonOk returns a tuple with the Balloon field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetBalloon

`func (o *VmConfig) SetBalloon(v BalloonConfig)`

SetBalloon sets Balloon field to given value.

### HasBalloon

`func (o *VmConfig) HasBalloon() bool`

HasBalloon returns a boolean if a field has been set.

### GetFs

`func (o *VmConfig) GetFs() []FsConfig`

GetFs returns the Fs field if non-nil, zero value otherwise.

### GetFsOk

`func (o *VmConfig) GetFsOk() (*[]FsConfig, bool)`

GetFsOk returns a tuple with the Fs field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetFs

`func (o *VmConfig) SetFs(v []FsConfig)`

SetFs sets Fs field to given value.

### HasFs

`func (o *VmConfig) HasFs() bool`

HasFs returns a boolean if a field has been set.

### GetPmem

`func (o *VmConfig) GetPmem() []PmemConfig`

GetPmem returns the Pmem field if non-nil, zero value otherwise.

### GetPmemOk

`func (o *VmConfig) GetPmemOk() (*[]PmemConfig, bool)`

GetPmemOk returns a tuple with the Pmem field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPmem

`func (o *VmConfig) SetPmem(v []PmemConfig)`

SetPmem sets Pmem field to given value.

### HasPmem

`func (o *VmConfig) HasPmem() bool`

HasPmem returns a boolean if a field has been set.

### GetSerial

`func (o *VmConfig) GetSerial() ConsoleConfig`

GetSerial returns the Serial field if non-nil, zero value otherwise.

### GetSerialOk

`func (o *VmConfig) GetSerialOk() (*ConsoleConfig, bool)`

GetSerialOk returns a tuple with the Serial field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSerial

`func (o *VmConfig) SetSerial(v ConsoleConfig)`

SetSerial sets Serial field to given value.

### HasSerial

`func (o *VmConfig) HasSerial() bool`

HasSerial returns a boolean if a field has been set.

### GetConsole

`func (o *VmConfig) GetConsole() ConsoleConfig`

GetConsole returns the Console field if non-nil, zero value otherwise.

### GetConsoleOk

`func (o *VmConfig) GetConsoleOk() (*ConsoleConfig, bool)`

GetConsoleOk returns a tuple with the Console field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetConsole

`func (o *VmConfig) SetConsole(v ConsoleConfig)`

SetConsole sets Console field to given value.

### HasConsole

`func (o *VmConfig) HasConsole() bool`

HasConsole returns a boolean if a field has been set.

### GetDebugConsole

`func (o *VmConfig) GetDebugConsole() DebugConsoleConfig`

GetDebugConsole returns the DebugConsole field if non-nil, zero value otherwise.

### GetDebugConsoleOk

`func (o *VmConfig) GetDebugConsoleOk() (*DebugConsoleConfig, bool)`

GetDebugConsoleOk returns a tuple with the DebugConsole field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDebugConsole

`func (o *VmConfig) SetDebugConsole(v DebugConsoleConfig)`

SetDebugConsole sets DebugConsole field to given value.

### HasDebugConsole

`func (o *VmConfig) HasDebugConsole() bool`

HasDebugConsole returns a boolean if a field has been set.

### GetDevices

`func (o *VmConfig) GetDevices() []DeviceConfig`

GetDevices returns the Devices field if non-nil, zero value otherwise.

### GetDevicesOk

`func (o *VmConfig) GetDevicesOk() (*[]DeviceConfig, bool)`

GetDevicesOk returns a tuple with the Devices field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetDevices

`func (o *VmConfig) SetDevices(v []DeviceConfig)`

SetDevices sets Devices field to given value.

### HasDevices

`func (o *VmConfig) HasDevices() bool`

HasDevices returns a boolean if a field has been set.

### GetVdpa

`func (o *VmConfig) GetVdpa() []VdpaConfig`

GetVdpa returns the Vdpa field if non-nil, zero value otherwise.

### GetVdpaOk

`func (o *VmConfig) GetVdpaOk() (*[]VdpaConfig, bool)`

GetVdpaOk returns a tuple with the Vdpa field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVdpa

`func (o *VmConfig) SetVdpa(v []VdpaConfig)`

SetVdpa sets Vdpa field to given value.

### HasVdpa

`func (o *VmConfig) HasVdpa() bool`

HasVdpa returns a boolean if a field has been set.

### GetVsock

`func (o *VmConfig) GetVsock() VsockConfig`

GetVsock returns the Vsock field if non-nil, zero value otherwise.

### GetVsockOk

`func (o *VmConfig) GetVsockOk() (*VsockConfig, bool)`

GetVsockOk returns a tuple with the Vsock field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetVsock

`func (o *VmConfig) SetVsock(v VsockConfig)`

SetVsock sets Vsock field to given value.

### HasVsock

`func (o *VmConfig) HasVsock() bool`

HasVsock returns a boolean if a field has been set.

### GetSgxEpc

`func (o *VmConfig) GetSgxEpc() []SgxEpcConfig`

GetSgxEpc returns the SgxEpc field if non-nil, zero value otherwise.

### GetSgxEpcOk

`func (o *VmConfig) GetSgxEpcOk() (*[]SgxEpcConfig, bool)`

GetSgxEpcOk returns a tuple with the SgxEpc field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetSgxEpc

`func (o *VmConfig) SetSgxEpc(v []SgxEpcConfig)`

SetSgxEpc sets SgxEpc field to given value.

### HasSgxEpc

`func (o *VmConfig) HasSgxEpc() bool`

HasSgxEpc returns a boolean if a field has been set.

### GetNuma

`func (o *VmConfig) GetNuma() []NumaConfig`

GetNuma returns the Numa field if non-nil, zero value otherwise.

### GetNumaOk

`func (o *VmConfig) GetNumaOk() (*[]NumaConfig, bool)`

GetNumaOk returns a tuple with the Numa field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetNuma

`func (o *VmConfig) SetNuma(v []NumaConfig)`

SetNuma sets Numa field to given value.

### HasNuma

`func (o *VmConfig) HasNuma() bool`

HasNuma returns a boolean if a field has been set.

### GetIommu

`func (o *VmConfig) GetIommu() bool`

GetIommu returns the Iommu field if non-nil, zero value otherwise.

### GetIommuOk

`func (o *VmConfig) GetIommuOk() (*bool, bool)`

GetIommuOk returns a tuple with the Iommu field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetIommu

`func (o *VmConfig) SetIommu(v bool)`

SetIommu sets Iommu field to given value.

### HasIommu

`func (o *VmConfig) HasIommu() bool`

HasIommu returns a boolean if a field has been set.

### GetWatchdog

`func (o *VmConfig) GetWatchdog() bool`

GetWatchdog returns the Watchdog field if non-nil, zero value otherwise.

### GetWatchdogOk

`func (o *VmConfig) GetWatchdogOk() (*bool, bool)`

GetWatchdogOk returns a tuple with the Watchdog field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetWatchdog

`func (o *VmConfig) SetWatchdog(v bool)`

SetWatchdog sets Watchdog field to given value.

### HasWatchdog

`func (o *VmConfig) HasWatchdog() bool`

HasWatchdog returns a boolean if a field has been set.

### GetPvpanic

`func (o *VmConfig) GetPvpanic() bool`

GetPvpanic returns the Pvpanic field if non-nil, zero value otherwise.

### GetPvpanicOk

`func (o *VmConfig) GetPvpanicOk() (*bool, bool)`

GetPvpanicOk returns a tuple with the Pvpanic field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPvpanic

`func (o *VmConfig) SetPvpanic(v bool)`

SetPvpanic sets Pvpanic field to given value.

### HasPvpanic

`func (o *VmConfig) HasPvpanic() bool`

HasPvpanic returns a boolean if a field has been set.

### GetPciSegments

`func (o *VmConfig) GetPciSegments() []PciSegmentConfig`

GetPciSegments returns the PciSegments field if non-nil, zero value otherwise.

### GetPciSegmentsOk

`func (o *VmConfig) GetPciSegmentsOk() (*[]PciSegmentConfig, bool)`

GetPciSegmentsOk returns a tuple with the PciSegments field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPciSegments

`func (o *VmConfig) SetPciSegments(v []PciSegmentConfig)`

SetPciSegments sets PciSegments field to given value.

### HasPciSegments

`func (o *VmConfig) HasPciSegments() bool`

HasPciSegments returns a boolean if a field has been set.

### GetPlatform

`func (o *VmConfig) GetPlatform() PlatformConfig`

GetPlatform returns the Platform field if non-nil, zero value otherwise.

### GetPlatformOk

`func (o *VmConfig) GetPlatformOk() (*PlatformConfig, bool)`

GetPlatformOk returns a tuple with the Platform field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetPlatform

`func (o *VmConfig) SetPlatform(v PlatformConfig)`

SetPlatform sets Platform field to given value.

### HasPlatform

`func (o *VmConfig) HasPlatform() bool`

HasPlatform returns a boolean if a field has been set.

### GetTpm

`func (o *VmConfig) GetTpm() TpmConfig`

GetTpm returns the Tpm field if non-nil, zero value otherwise.

### GetTpmOk

`func (o *VmConfig) GetTpmOk() (*TpmConfig, bool)`

GetTpmOk returns a tuple with the Tpm field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetTpm

`func (o *VmConfig) SetTpm(v TpmConfig)`

SetTpm sets Tpm field to given value.

### HasTpm

`func (o *VmConfig) HasTpm() bool`

HasTpm returns a boolean if a field has been set.

### GetLandlockEnable

`func (o *VmConfig) GetLandlockEnable() bool`

GetLandlockEnable returns the LandlockEnable field if non-nil, zero value otherwise.

### GetLandlockEnableOk

`func (o *VmConfig) GetLandlockEnableOk() (*bool, bool)`

GetLandlockEnableOk returns a tuple with the LandlockEnable field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetLandlockEnable

`func (o *VmConfig) SetLandlockEnable(v bool)`

SetLandlockEnable sets LandlockEnable field to given value.

### HasLandlockEnable

`func (o *VmConfig) HasLandlockEnable() bool`

HasLandlockEnable returns a boolean if a field has been set.

### GetLandlockRules

`func (o *VmConfig) GetLandlockRules() []LandlockConfig`

GetLandlockRules returns the LandlockRules field if non-nil, zero value otherwise.

### GetLandlockRulesOk

`func (o *VmConfig) GetLandlockRulesOk() (*[]LandlockConfig, bool)`

GetLandlockRulesOk returns a tuple with the LandlockRules field if it's non-nil, zero value otherwise
and a boolean to check if the value has been set.

### SetLandlockRules

`func (o *VmConfig) SetLandlockRules(v []LandlockConfig)`

SetLandlockRules sets LandlockRules field to given value.

### HasLandlockRules

`func (o *VmConfig) HasLandlockRules() bool`

HasLandlockRules returns a boolean if a field has been set.


[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


