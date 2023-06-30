//go:build linux

// Copyright (c) 2019 Ericsson Eurolab Deutschland G.m.b.H.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	chclient "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/cloud-hypervisor/client"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/pkg/errors"
	"github.com/stretchr/testify/assert"
)

const (
	FAIL = true
	PASS = !FAIL
)

func newClhConfig() (HypervisorConfig, error) {

	setupClh()

	if testClhPath == "" {
		return HypervisorConfig{}, errors.New("hypervisor fake path is empty")
	}

	if testVirtiofsdPath == "" {
		return HypervisorConfig{}, errors.New("virtiofsd fake path is empty")
	}

	if _, err := os.Stat(testClhPath); os.IsNotExist(err) {
		return HypervisorConfig{}, err
	}

	if _, err := os.Stat(testVirtiofsdPath); os.IsNotExist(err) {
		return HypervisorConfig{}, err
	}

	return HypervisorConfig{
		KernelPath:                    testClhKernelPath,
		ImagePath:                     testClhImagePath,
		RootfsType:                    string(EXT4),
		HypervisorPath:                testClhPath,
		NumVCPUsF:                     defaultVCPUs,
		BlockDeviceDriver:             config.VirtioBlock,
		MemorySize:                    defaultMemSzMiB,
		DefaultBridges:                defaultBridges,
		DefaultMaxVCPUs:               uint32(64),
		SharedFS:                      config.VirtioFS,
		VirtioFSCache:                 typeVirtioFSCacheModeAlways,
		VirtioFSDaemon:                testVirtiofsdPath,
		NetRateLimiterBwMaxRate:       int64(0),
		NetRateLimiterBwOneTimeBurst:  int64(0),
		NetRateLimiterOpsMaxRate:      int64(0),
		NetRateLimiterOpsOneTimeBurst: int64(0),
		HotPlugVFIO:                   config.NoPort,
	}, nil
}

type clhClientMock struct {
	vmInfo chclient.VmInfo
}

func (c *clhClientMock) VmmPingGet(ctx context.Context) (chclient.VmmPingResponse, *http.Response, error) {
	return chclient.VmmPingResponse{}, nil, nil
}

func (c *clhClientMock) ShutdownVMM(ctx context.Context) (*http.Response, error) {
	return nil, nil
}

func (c *clhClientMock) CreateVM(ctx context.Context, vmConfig chclient.VmConfig) (*http.Response, error) {
	c.vmInfo.State = clhStateCreated
	return nil, nil
}

//nolint:golint
func (c *clhClientMock) VmInfoGet(ctx context.Context) (chclient.VmInfo, *http.Response, error) {
	return c.vmInfo, nil, nil
}

func (c *clhClientMock) BootVM(ctx context.Context) (*http.Response, error) {
	c.vmInfo.State = clhStateRunning
	return nil, nil
}

//nolint:golint
func (c *clhClientMock) VmResizePut(ctx context.Context, vmResize chclient.VmResize) (*http.Response, error) {
	return nil, nil
}

//nolint:golint
func (c *clhClientMock) VmAddDevicePut(ctx context.Context, deviceConfig chclient.DeviceConfig) (chclient.PciDeviceInfo, *http.Response, error) {
	return chclient.PciDeviceInfo{}, nil, nil
}

//nolint:golint
func (c *clhClientMock) VmAddDiskPut(ctx context.Context, diskConfig chclient.DiskConfig) (chclient.PciDeviceInfo, *http.Response, error) {
	return chclient.PciDeviceInfo{Bdf: "0000:00:0a.0"}, nil, nil
}

//nolint:golint
func (c *clhClientMock) VmRemoveDevicePut(ctx context.Context, vmRemoveDevice chclient.VmRemoveDevice) (*http.Response, error) {
	return nil, nil
}

func TestCloudHypervisorAddVSock(t *testing.T) {
	assert := assert.New(t)
	clh := cloudHypervisor{}

	clh.addVSock(1, "path")
	assert.Equal(clh.vmconfig.Vsock.Cid, int64(1))
	assert.Equal(clh.vmconfig.Vsock.Socket, "path")
}

// Check addNet appends to the network config list new configurations.
// Check that the elements in the list has the correct values
func TestCloudHypervisorAddNetCheckNetConfigListValues(t *testing.T) {
	assert := assert.New(t)

	macTest := "00:00:00:00:00"

	file, err := os.CreateTemp("", "netFd")
	assert.Nil(err)
	defer os.Remove(file.Name())

	vmFds := make([]*os.File, 1)
	vmFds = append(vmFds, file)

	clh := cloudHypervisor{}
	clh.netDevicesFiles = make(map[string][]*os.File)

	e := &VethEndpoint{}
	e.NetPair.TAPIface.HardAddr = macTest
	e.NetPair.TapInterface.VMFds = vmFds

	err = clh.addNet(e)
	assert.Nil(err)

	assert.Equal(len(*clh.netDevices), 1)
	if err == nil {
		assert.Equal(*(*clh.netDevices)[0].Mac, macTest)
	}

	err = clh.addNet(e)
	assert.Nil(err)

	assert.Equal(len(*clh.netDevices), 2)
	if err == nil {
		assert.Equal(*(*clh.netDevices)[1].Mac, macTest)
	}
}

// Check addNet with valid values, and fail with invalid values
// For Cloud Hypervisor only tap is be required
func TestCloudHypervisorAddNetCheckEnpointTypes(t *testing.T) {
	assert := assert.New(t)

	macTest := "00:00:00:00:00"

	file, err := os.CreateTemp("", "netFd")
	assert.Nil(err)
	defer os.Remove(file.Name())

	vmFds := make([]*os.File, 1)
	vmFds = append(vmFds, file)

	validVeth := &VethEndpoint{}
	validVeth.NetPair.TAPIface.HardAddr = macTest
	validVeth.NetPair.TapInterface.VMFds = vmFds

	type args struct {
		e Endpoint
	}
	// nolint: govet
	tests := []struct {
		name    string
		args    args
		wantErr bool
	}{
		{"TapEndpoint", args{e: &TapEndpoint{}}, true},
		{"Empty VethEndpoint", args{e: &VethEndpoint{}}, true},
		{"Valid VethEndpoint", args{e: validVeth}, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			clh := &cloudHypervisor{}
			clh.netDevicesFiles = make(map[string][]*os.File)
			if err := clh.addNet(tt.args.e); (err != nil) != tt.wantErr {
				t.Errorf("cloudHypervisor.addNet() error = %v, wantErr %v", err, tt.wantErr)
			} else if err == nil {
				files := clh.netDevicesFiles[macTest]
				assert.Equal(files, vmFds)
			}
		})
	}
}

// Check AddNet properly sets up the network rate limiter
func TestCloudHypervisorNetRateLimiter(t *testing.T) {
	assert := assert.New(t)

	file, err := os.CreateTemp("", "netFd")
	assert.Nil(err)
	defer os.Remove(file.Name())

	vmFds := make([]*os.File, 1)
	vmFds = append(vmFds, file)

	validVeth := &VethEndpoint{}
	validVeth.NetPair.TapInterface.VMFds = vmFds

	type args struct {
		bwMaxRate       int64
		bwOneTimeBurst  int64
		opsMaxRate      int64
		opsOneTimeBurst int64
	}

	//nolint: govet
	tests := []struct {
		name                  string
		args                  args
		expectsRateLimiter    bool
		expectsBwBucketToken  bool
		expectsOpsBucketToken bool
	}{
		// Bandwidth
		{
			"Bandwidth | max rate with one time burst",
			args{
				bwMaxRate:      int64(1000),
				bwOneTimeBurst: int64(10000),
			},
			true,  // expectsRateLimiter
			true,  // expectsBwBucketToken
			false, // expectsOpsBucketToken
		},
		{
			"Bandwidth | max rate without one time burst",
			args{
				bwMaxRate: int64(1000),
			},
			true,  // expectsRateLimiter
			true,  // expectsBwBucketToken
			false, // expectsOpsBucketToken
		},
		{
			"Bandwidth | no max rate with one time burst",
			args{
				bwOneTimeBurst: int64(10000),
			},
			false, // expectsRateLimiter
			false, // expectsBwBucketToken
			false, // expectsOpsBucketToken
		},
		{
			"Bandwidth | no max rate and no one time burst",
			args{},
			false, // expectsRateLimiter
			false, // expectsBwBucketToken
			false, // expectsOpsBucketToken
		},

		// Operations
		{
			"Operations | max rate with one time burst",
			args{
				opsMaxRate:      int64(1000),
				opsOneTimeBurst: int64(10000),
			},
			true,  // expectsRateLimiter
			false, // expectsBwBucketToken
			true,  // expectsOpsBucketToken
		},
		{
			"Operations | max rate without one time burst",
			args{
				opsMaxRate: int64(1000),
			},
			true,  // expectsRateLimiter
			false, // expectsBwBucketToken
			true,  // expectsOpsBucketToken
		},
		{
			"Operations | no max rate with one time burst",
			args{
				opsOneTimeBurst: int64(10000),
			},
			false, // expectsRateLimiter
			false, // expectsBwBucketToken
			false, // expectsOpsBucketToken
		},
		{
			"Operations | no max rate and no one time burst",
			args{},
			false, // expectsRateLimiter
			false, // expectsBwBucketToken
			false, // expectsOpsBucketToken
		},

		// Bandwidth and Operations
		{
			"Bandwidth and Operations | max rate with one time burst",
			args{
				bwMaxRate:       int64(1000),
				bwOneTimeBurst:  int64(10000),
				opsMaxRate:      int64(1000),
				opsOneTimeBurst: int64(10000),
			},
			true, // expectsRateLimiter
			true, // expectsBwBucketToken
			true, // expectsOpsBucketToken
		},
		{
			"Bandwidth and Operations | max rate without one time burst",
			args{
				bwMaxRate:  int64(1000),
				opsMaxRate: int64(1000),
			},
			true, // expectsRateLimiter
			true, // expectsBwBucketToken
			true, // expectsOpsBucketToken
		},
		{
			"Bandwidth and Operations | no max rate with one time burst",
			args{
				bwOneTimeBurst:  int64(10000),
				opsOneTimeBurst: int64(10000),
			},
			false, // expectsRateLimiter
			false, // expectsBwBucketToken
			false, // expectsOpsBucketToken
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			clhConfig, err := newClhConfig()
			assert.NoError(err)

			clhConfig.NetRateLimiterBwMaxRate = tt.args.bwMaxRate
			clhConfig.NetRateLimiterBwOneTimeBurst = tt.args.bwOneTimeBurst
			clhConfig.NetRateLimiterOpsMaxRate = tt.args.opsMaxRate
			clhConfig.NetRateLimiterOpsOneTimeBurst = tt.args.opsOneTimeBurst

			clh := &cloudHypervisor{}
			clh.netDevicesFiles = make(map[string][]*os.File)
			clh.config = clhConfig
			clh.APIClient = &clhClientMock{}

			if err := clh.addNet(validVeth); err != nil {
				t.Errorf("cloudHypervisor.addNet() error = %v", err)
			} else {
				netConfig := (*clh.netDevices)[0]

				assert.Equal(netConfig.HasRateLimiterConfig(), tt.expectsRateLimiter)
				if tt.expectsRateLimiter {
					rateLimiterConfig := netConfig.GetRateLimiterConfig()
					assert.Equal(rateLimiterConfig.HasBandwidth(), tt.expectsBwBucketToken)
					assert.Equal(rateLimiterConfig.HasOps(), tt.expectsOpsBucketToken)

					if tt.expectsBwBucketToken {
						bwBucketToken := rateLimiterConfig.GetBandwidth()
						assert.Equal(bwBucketToken.GetSize(), int64(utils.RevertBytes(uint64(tt.args.bwMaxRate/8))))
						assert.Equal(bwBucketToken.GetOneTimeBurst(), int64(utils.RevertBytes(uint64(tt.args.bwOneTimeBurst/8))))
					}

					if tt.expectsOpsBucketToken {
						opsBucketToken := rateLimiterConfig.GetOps()
						assert.Equal(opsBucketToken.GetSize(), int64(tt.args.opsMaxRate))
						assert.Equal(opsBucketToken.GetOneTimeBurst(), int64(tt.args.opsOneTimeBurst))
					}
				}
			}
		})
	}
}

func TestCloudHypervisorBootVM(t *testing.T) {
	clh := &cloudHypervisor{}
	clh.APIClient = &clhClientMock{}

	savedVmAddNetPutRequestFunc := vmAddNetPutRequest
	vmAddNetPutRequest = func(clh *cloudHypervisor) error { return nil }
	defer func() {
		vmAddNetPutRequest = savedVmAddNetPutRequestFunc
	}()

	var ctx context.Context
	if err := clh.bootVM(ctx); err != nil {
		t.Errorf("cloudHypervisor.bootVM() error = %v", err)
	}
}

func TestCloudHypervisorCleanupVM(t *testing.T) {
	assert := assert.New(t)
	store, err := persist.GetDriver()
	assert.NoError(err, "persist.GetDriver() unexpected error")

	clh := &cloudHypervisor{
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
	}

	err = clh.cleanupVM(true)
	assert.Error(err, "persist.GetDriver() expected error")

	clh.id = "cleanVMID"
	clh.config.VMid = "cleanVMID"

	err = clh.cleanupVM(true)
	assert.NoError(err, "persist.GetDriver() unexpected error")

	dir := filepath.Join(store.RunVMStoragePath(), clh.id)
	os.MkdirAll(dir, os.ModePerm)

	err = clh.cleanupVM(false)
	assert.NoError(err, "persist.GetDriver() unexpected error")

	_, err = os.Stat(dir)
	assert.Error(err, "dir should not exist %s", dir)

	assert.True(os.IsNotExist(err), "persist.GetDriver() unexpected error")
}

func TestClhCreateVM(t *testing.T) {
	assert := assert.New(t)

	store, err := persist.GetDriver()
	assert.NoError(err)

	network, err := NewNetwork()
	assert.NoError(err)

	clh := &cloudHypervisor{
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
	}

	config0, err := newClhConfig()
	assert.NoError(err)

	config1, err := newClhConfig()
	assert.NoError(err)
	config1.ImagePath = ""
	config1.InitrdPath = testClhInitrdPath

	config2, err := newClhConfig()
	assert.NoError(err)
	config2.Debug = true

	config3, err := newClhConfig()
	assert.NoError(err)
	config3.Debug = true
	config3.ConfidentialGuest = true

	config4, err := newClhConfig()
	assert.NoError(err)
	config4.SGXEPCSize = 1

	config5, err := newClhConfig()
	assert.NoError(err)
	config5.SharedFS = config.VirtioFSNydus

	type testData struct {
		config      HypervisorConfig
		expectError bool
		configMatch bool
	}

	data := []testData{
		{config0, false, true},
		{config1, false, true},
		{config2, false, true},
		{config3, true, false},
		{config4, false, true},
		{config5, false, true},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]", i)

		err = clh.CreateVM(context.Background(), "testSandbox", network, &d.config)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)

		if d.configMatch {
			assert.Exactly(d.config, clh.config, msg)
		}
	}
}

func TestCloudHypervisorStartSandbox(t *testing.T) {
	assert := assert.New(t)
	clhConfig, err := newClhConfig()
	assert.NoError(err)
	clhConfig.Debug = true
	clhConfig.DisableSeccomp = true

	store, err := persist.GetDriver()
	assert.NoError(err)

	savedVmAddNetPutRequestFunc := vmAddNetPutRequest
	vmAddNetPutRequest = func(clh *cloudHypervisor) error { return nil }
	defer func() {
		vmAddNetPutRequest = savedVmAddNetPutRequestFunc
	}()

	clhConfig.VMStorePath = store.RunVMStoragePath()
	clhConfig.RunStorePath = store.RunStoragePath()

	clh := &cloudHypervisor{
		config:         clhConfig,
		APIClient:      &clhClientMock{},
		virtiofsDaemon: &virtiofsdMock{},
	}

	err = clh.StartVM(context.Background(), 10)
	assert.NoError(err)

	_, err = clh.loadVirtiofsDaemon("/tmp/xyzabc")
	assert.NoError(err)

	err = clh.stopVirtiofsDaemon(context.Background())
	assert.NoError(err)

	_, _, err = clh.GetVMConsole(context.Background(), "test")
	assert.NoError(err)

	_, err = clh.GetThreadIDs(context.Background())
	assert.NoError(err)

	assert.True(clh.getClhStopSandboxTimeout().Nanoseconds() != 0)

	pid := clh.GetPids()
	assert.True(pid[0] != 0)

	pid2 := *clh.GetVirtioFsPid()
	assert.True(pid2 == 0)

	mem := clh.GetTotalMemoryMB(context.Background())
	assert.True(mem == 0)

	err = clh.PauseVM(context.Background())
	assert.NoError(err)

	err = clh.SaveVM()
	assert.NoError(err)

	err = clh.ResumeVM(context.Background())
	assert.NoError(err)

	err = clh.Check()
	assert.NoError(err)

	err = clh.Cleanup(context.Background())
	assert.NoError(err)
}

func TestCloudHypervisorResizeMemory(t *testing.T) {
	assert := assert.New(t)
	clhConfig, err := newClhConfig()
	type args struct {
		reqMemMB          uint32
		memoryBlockSizeMB uint32
	}
	tests := []struct {
		name           string
		args           args
		expectedMemDev MemoryDevice
		wantErr        bool
	}{
		{"Resize to zero", args{0, 128}, MemoryDevice{Probe: false, SizeMB: 0}, FAIL},
		{"Resize to aligned size", args{clhConfig.MemorySize + 128, 128}, MemoryDevice{Probe: false, SizeMB: 128}, PASS},
		{"Resize to aligned size", args{clhConfig.MemorySize + 129, 128}, MemoryDevice{Probe: false, SizeMB: 256}, PASS},
		{"Resize to NOT aligned size", args{clhConfig.MemorySize + 125, 128}, MemoryDevice{Probe: false, SizeMB: 128}, PASS},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			assert.NoError(err)
			clh := cloudHypervisor{}

			mockClient := &clhClientMock{}
			mockClient.vmInfo.Config = *chclient.NewVmConfig(*chclient.NewPayloadConfig())
			mockClient.vmInfo.Config.Memory = chclient.NewMemoryConfig(int64(utils.MemUnit(clhConfig.MemorySize) * utils.MiB))
			mockClient.vmInfo.Config.Memory.HotplugSize = func(i int64) *int64 { return &i }(int64(40 * utils.GiB.ToBytes()))

			clh.APIClient = mockClient
			clh.config = clhConfig

			newMem, memDev, err := clh.ResizeMemory(context.Background(), tt.args.reqMemMB, tt.args.memoryBlockSizeMB, false)

			if (err != nil) != tt.wantErr {
				t.Errorf("cloudHypervisor.ResizeMemory() error = %v, expected to fail = %v", err, tt.wantErr)
				return
			}

			if err != nil {
				return
			}

			expectedMem := clhConfig.MemorySize + uint32(tt.expectedMemDev.SizeMB)

			if newMem != expectedMem {
				t.Errorf("cloudHypervisor.ResizeMemory() got = %+v, want %+v", newMem, expectedMem)
			}

			if !reflect.DeepEqual(memDev, tt.expectedMemDev) {
				t.Errorf("cloudHypervisor.ResizeMemory() got = %+v, want %+v", memDev, tt.expectedMemDev)
			}
		})
	}
}

func TestCloudHypervisorHotplugAddBlockDevice(t *testing.T) {
	assert := assert.New(t)

	clhConfig, err := newClhConfig()
	assert.NoError(err)

	clh := &cloudHypervisor{}
	clh.config = clhConfig
	clh.APIClient = &clhClientMock{}
	clh.devicesIds = make(map[string]string)

	clh.config.BlockDeviceDriver = config.VirtioBlock
	err = clh.hotplugAddBlockDevice(&config.BlockDrive{Pmem: false})
	assert.NoError(err, "Hotplug disk block device expected no error")

	err = clh.hotplugAddBlockDevice(&config.BlockDrive{Pmem: true})
	assert.Error(err, "Hotplug pmem block device expected error")

	clh.config.BlockDeviceDriver = config.VirtioSCSI
	err = clh.hotplugAddBlockDevice(&config.BlockDrive{Pmem: false})
	assert.Error(err, "Hotplug block device not using 'virtio-blk' expected error")
}

func TestCloudHypervisorHotplugRemoveDevice(t *testing.T) {
	assert := assert.New(t)

	clhConfig, err := newClhConfig()
	assert.NoError(err)

	clh := &cloudHypervisor{}
	clh.config = clhConfig
	clh.APIClient = &clhClientMock{}
	clh.devicesIds = make(map[string]string)

	_, err = clh.HotplugRemoveDevice(context.Background(), &config.BlockDrive{}, BlockDev)
	assert.NoError(err, "Hotplug remove block device expected no error")

	_, err = clh.HotplugRemoveDevice(context.Background(), &config.VFIODev{}, VfioDev)
	assert.NoError(err, "Hotplug remove vfio block device expected no error")

	_, err = clh.HotplugRemoveDevice(context.Background(), nil, NetDev)
	assert.Error(err, "Hotplug remove pmem block device expected error")
}

func TestClhGenerateSocket(t *testing.T) {
	assert := assert.New(t)

	// Ensure the type is fully constructed
	hypervisor, err := NewHypervisor("clh")
	assert.NoError(err)

	clh, ok := hypervisor.(*cloudHypervisor)
	assert.True(ok)

	clh.config = HypervisorConfig{
		VMStorePath:  "/foo",
		RunStorePath: "/bar",
	}

	clh.addVSock(1, "path")

	s, err := clh.GenerateSocket("c")

	assert.NoError(err)
	assert.NotNil(s)

	hvsock, ok := s.(types.HybridVSock)
	assert.True(ok)
	assert.NotEmpty(hvsock.UdsPath)

	// Path must be absolute
	assert.True(strings.HasPrefix(hvsock.UdsPath, "/"), "failed: socket path: %s", hvsock.UdsPath)

	assert.NotZero(hvsock.Port)
}

func TestClhSetConfig(t *testing.T) {
	assert := assert.New(t)

	config, err := newClhConfig()
	assert.NoError(err)

	clh := &cloudHypervisor{}
	assert.Equal(clh.config, HypervisorConfig{})

	err = clh.setConfig(&config)
	assert.NoError(err)

	assert.Equal(clh.config, config)
}

func TestClhCapabilities(t *testing.T) {
	assert := assert.New(t)

	hConfig, err := newClhConfig()
	assert.NoError(err)

	clh := &cloudHypervisor{}
	assert.Equal(clh.config, HypervisorConfig{})

	hConfig.SharedFS = config.VirtioFS

	err = clh.setConfig(&hConfig)
	assert.NoError(err)

	var ctx context.Context
	c := clh.Capabilities(ctx)
	assert.True(c.IsFsSharingSupported())

	hConfig.SharedFS = config.NoSharedFS

	err = clh.setConfig(&hConfig)
	assert.NoError(err)

	c = clh.Capabilities(ctx)
	assert.False(c.IsFsSharingSupported())

	assert.True(c.IsNetworkDeviceHotplugSupported())
	assert.True(c.IsBlockDeviceHotplugSupported())
}
