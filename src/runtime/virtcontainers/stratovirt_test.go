//go:build linux

// Copyright (c) 2023 Huawei Technologies Co.,Ltd.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/pkg/errors"
	"github.com/stretchr/testify/assert"
)

func newStratovirtConfig() (HypervisorConfig, error) {

	setupStratovirt()

	if testStratovirtPath == "" {
		return HypervisorConfig{}, errors.New("hypervisor fake path is empty")
	}

	if testVirtiofsdPath == "" {
		return HypervisorConfig{}, errors.New("virtiofsd fake path is empty")
	}

	if _, err := os.Stat(testStratovirtPath); os.IsNotExist(err) {
		return HypervisorConfig{}, err
	}

	if _, err := os.Stat(testVirtiofsdPath); os.IsNotExist(err) {
		return HypervisorConfig{}, err
	}

	return HypervisorConfig{
		HypervisorPath:    testStratovirtPath,
		KernelPath:        testStratovirtKernelPath,
		InitrdPath:        testStratovirtInitrdPath,
		RootfsType:        string(EXT4),
		NumVCPUsF:         defaultVCPUs,
		BlockDeviceDriver: config.VirtioBlock,
		MemorySize:        defaultMemSzMiB,
		DefaultMaxVCPUs:   uint32(64),
		SharedFS:          config.VirtioFS,
		VirtioFSCache:     typeVirtioFSCacheModeAlways,
		VirtioFSDaemon:    testVirtiofsdPath,
	}, nil
}

func TestStratovirtCreateVM(t *testing.T) {
	assert := assert.New(t)

	store, err := persist.GetDriver()
	assert.NoError(err)

	network, err := NewNetwork()
	assert.NoError(err)

	sv := stratovirt{
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
	}

	config0, err := newStratovirtConfig()
	assert.NoError(err)

	config1, err := newStratovirtConfig()
	assert.NoError(err)
	config1.ImagePath = testStratovirtImagePath
	config1.InitrdPath = ""

	config2, err := newStratovirtConfig()
	assert.NoError(err)
	config2.Debug = true

	config3, err := newStratovirtConfig()
	assert.NoError(err)
	config3.SharedFS = config.VirtioFS

	config4, err := newStratovirtConfig()
	assert.NoError(err)
	config4.SharedFS = config.VirtioFSNydus

	type testData struct {
		config      HypervisorConfig
		expectError bool
		configMatch bool
	}

	data := []testData{
		{config0, false, true},
		{config1, false, true},
		{config2, false, true},
		{config3, false, true},
		{config4, false, true},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]", i)

		err = sv.CreateVM(context.Background(), "testSandbox", network, &d.config)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)

		if d.configMatch {
			assert.Exactly(d.config, sv.config, msg)
		}
	}
}

func TestStratovirtStartSandbox(t *testing.T) {
	assert := assert.New(t)
	sConfig, err := newStratovirtConfig()
	assert.NoError(err)
	sConfig.Debug = true

	network, err := NewNetwork()
	assert.NoError(err)

	store, err := persist.GetDriver()
	assert.NoError(err)

	sConfig.VMStorePath = store.RunVMStoragePath()
	sConfig.RunStorePath = store.RunStoragePath()

	sv := &stratovirt{
		config:         sConfig,
		virtiofsDaemon: &virtiofsdMock{},
	}

	assert.Exactly(sv.stopped.Load(), false)

	err = sv.CreateVM(context.Background(), "testSandbox", network, &sConfig)
	assert.NoError(err)

	mem := sv.GetTotalMemoryMB(context.Background())
	assert.True(mem > 0)

	err = sv.StartVM(context.Background(), 10)
	assert.Error(err)
}

func TestStratovirtCleanupVM(t *testing.T) {
	assert := assert.New(t)
	store, err := persist.GetDriver()
	assert.NoError(err, "persist.GetDriver() unexpected error")

	sv := &stratovirt{
		id: "cleanVM",
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
	}
	sv.svConfig.vmPath = filepath.Join(sv.config.VMStorePath, sv.id)
	sv.config.VMid = "cleanVM"

	err = sv.cleanupVM(true)
	assert.NoError(err, "persist.GetDriver() unexpected error")

	dir := filepath.Join(store.RunVMStoragePath(), sv.id)
	os.MkdirAll(dir, os.ModePerm)

	err = sv.cleanupVM(false)
	assert.NoError(err, "persist.GetDriver() unexpected error")

	_, err = os.Stat(dir)
	assert.Error(err, "dir should not exist %s", dir)

	assert.True(os.IsNotExist(err), "persist.GetDriver() unexpected error")
}

func TestStratovirtAddFsDevice(t *testing.T) {
	assert := assert.New(t)
	sConfig, err := newStratovirtConfig()
	assert.NoError(err)
	sConfig.SharedFS = config.VirtioFS
	mountTag := "testMountTag"

	sv := &stratovirt{
		ctx:    context.Background(),
		config: sConfig,
	}
	volume := types.Volume{
		MountTag: mountTag,
	}
	expected := []VirtioDev{
		virtioFs{
			backend:  "socket",
			charID:   "virtio_fs",
			charDev:  "virtio_fs",
			tag:      volume.MountTag,
			deviceID: "virtio-fs0",
			driver:   mmioBus,
		},
	}

	err = sv.AddDevice(context.Background(), volume, FsDev)
	assert.NoError(err)
	assert.Exactly(sv.svConfig.devices, expected)
}

func TestStratovirtAddBlockDevice(t *testing.T) {
	assert := assert.New(t)
	sConfig, err := newStratovirtConfig()
	assert.NoError(err)

	sv := &stratovirt{
		ctx:    context.Background(),
		config: sConfig,
	}
	blockDrive := config.BlockDrive{}
	expected := []VirtioDev{
		blkDevice{
			id:       "rootfs",
			filePath: sv.svConfig.rootfsPath,
			deviceID: "virtio-blk0",
			driver:   mmioBus,
		},
	}

	err = sv.AddDevice(context.Background(), blockDrive, BlockDev)
	assert.NoError(err)
	assert.Exactly(sv.svConfig.devices, expected)
}

func TestStratovirtAddVsockDevice(t *testing.T) {
	assert := assert.New(t)
	sConfig, err := newStratovirtConfig()
	assert.NoError(err)

	dir := t.TempDir()
	vsockFilename := filepath.Join(dir, "vsock")
	contextID := uint64(3)
	port := uint32(1024)
	vsockFile, fileErr := os.Create(vsockFilename)
	assert.NoError(fileErr)
	defer vsockFile.Close()

	sv := &stratovirt{
		ctx:    context.Background(),
		config: sConfig,
	}
	vsock := types.VSock{
		ContextID: contextID,
		Port:      port,
		VhostFd:   vsockFile,
	}
	expected := []VirtioDev{
		vhostVsock{
			id:      "vsock-id",
			guestID: fmt.Sprintf("%d", contextID),
			VHostFD: vsockFile,
			driver:  mmioBus,
		},
	}

	err = sv.AddDevice(context.Background(), vsock, VSockPCIDev)
	assert.NoError(err)
	assert.Exactly(sv.svConfig.devices, expected)
}

func TestStratovirtAddConsole(t *testing.T) {
	assert := assert.New(t)
	sConfig, err := newStratovirtConfig()
	assert.NoError(err)

	sv := &stratovirt{
		ctx:    context.Background(),
		config: sConfig,
	}
	sock := types.Socket{}
	expected := []VirtioDev{
		consoleDevice{
			id:       "virtio-serial0",
			backend:  "socket",
			charID:   "charconsole0",
			devType:  "virtconsole",
			charDev:  "charconsole0",
			deviceID: "virtio-console0",
			driver:   mmioBus,
		},
	}

	err = sv.AddDevice(context.Background(), sock, SerialPortDev)
	assert.NoError(err)
	assert.Exactly(sv.svConfig.devices, expected)
}

func TestStratovirtGetSandboxConsole(t *testing.T) {
	assert := assert.New(t)
	store, err := persist.GetDriver()
	assert.NoError(err)

	sandboxID := "testSandboxID"
	sv := &stratovirt{
		id:  sandboxID,
		ctx: context.Background(),
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
	}
	expected := filepath.Join(store.RunVMStoragePath(), sandboxID, debugSocket)

	proto, result, err := sv.GetVMConsole(sv.ctx, sandboxID)
	assert.NoError(err)
	assert.Equal(result, expected)
	assert.Equal(proto, consoleProtoUnix)
}

func TestStratovirtCapabilities(t *testing.T) {
	assert := assert.New(t)

	sConfig, err := newStratovirtConfig()
	assert.NoError(err)

	sv := stratovirt{}
	assert.Equal(sv.config, HypervisorConfig{})

	sConfig.SharedFS = config.VirtioFS

	err = sv.setConfig(&sConfig)
	assert.NoError(err)

	var ctx context.Context
	c := sv.Capabilities(ctx)
	assert.True(c.IsFsSharingSupported())

	sConfig.SharedFS = config.NoSharedFS

	err = sv.setConfig(&sConfig)
	assert.NoError(err)

	c = sv.Capabilities(ctx)
	assert.False(c.IsFsSharingSupported())
}

func TestStratovirtSetConfig(t *testing.T) {
	assert := assert.New(t)

	config, err := newStratovirtConfig()
	assert.NoError(err)

	sv := stratovirt{}
	assert.Equal(sv.config, HypervisorConfig{})

	err = sv.setConfig(&config)
	assert.NoError(err)

	assert.Equal(sv.config, config)
}

func TestStratovirtCleanup(t *testing.T) {
	assert := assert.New(t)
	sConfig, err := newStratovirtConfig()
	assert.NoError(err)

	sv := &stratovirt{
		ctx:    context.Background(),
		config: sConfig,
	}

	err = sv.Cleanup(sv.ctx)
	assert.Nil(err)
}

func TestStratovirtGetpids(t *testing.T) {
	assert := assert.New(t)

	sv := &stratovirt{}
	pids := sv.GetPids()
	assert.NotNil(pids)
	assert.True(len(pids) == 1)
	assert.True(pids[0] == 0)
}

func TestStratovirtBinPath(t *testing.T) {
	assert := assert.New(t)

	f, err := os.CreateTemp("", "stratovirt")
	assert.NoError(err)
	defer func() { _ = f.Close() }()
	defer func() { _ = os.Remove(f.Name()) }()

	expectedPath := f.Name()
	sConfig, err := newStratovirtConfig()
	assert.NoError(err)

	sConfig.HypervisorPath = expectedPath
	sv := &stratovirt{
		config: sConfig,
	}

	// get config hypervisor path
	path, err := sv.binPath()
	assert.NoError(err)
	assert.Equal(path, expectedPath)

	// config hypervisor path does not exist
	sv.config.HypervisorPath = "/abc/xyz/123"
	path, err = sv.binPath()
	assert.Error(err)
	assert.Equal(path, "")

	// get default stratovirt hypervisor path
	sv.config.HypervisorPath = ""
	path, err = sv.binPath()
	if _, errStat := os.Stat(path); os.IsNotExist(errStat) {
		assert.Error(err)
		assert.Equal(path, "")
	} else {
		assert.NoError(err)
		assert.Equal(path, defaultStratoVirt)
	}
}
