//go:build linux

// Copyright (c) 2016 Intel Corporation
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
	"github.com/kata-containers/kata-containers/src/runtime/pkg/govmm"
	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/pbnjay/memory"
	"github.com/pkg/errors"
	"github.com/stretchr/testify/assert"
)

func newQemuConfig() HypervisorConfig {
	return HypervisorConfig{
		KernelPath:          testQemuKernelPath,
		InitrdPath:          testQemuInitrdPath,
		HypervisorPath:      testQemuPath,
		NumVCPUsF:           defaultVCPUs,
		MemorySize:          defaultMemSzMiB,
		DefaultBridges:      defaultBridges,
		BlockDeviceDriver:   defaultBlockDriver,
		DefaultMaxVCPUs:     defaultMaxVCPUs,
		Msize9p:             defaultMsize9p,
		DisableGuestSeLinux: defaultDisableGuestSeLinux,
	}
}

func testQemuKernelParameters(t *testing.T, kernelParams []Param, expected string, debug bool) {
	qemuConfig := newQemuConfig()
	qemuConfig.KernelParams = kernelParams
	assert := assert.New(t)

	if debug == true {
		qemuConfig.Debug = true
	}

	q := &qemu{
		config: qemuConfig,
		arch:   &qemuArchBase{},
	}

	params := q.kernelParameters()
	assert.Equal(params, expected)
}

func TestQemuKernelParameters(t *testing.T) {
	expectedOut := fmt.Sprintf("panic=1 nr_cpus=%d selinux=0 foo=foo bar=bar", govmm.MaxVCPUs())
	params := []Param{
		{
			Key:   "foo",
			Value: "foo",
		},
		{
			Key:   "bar",
			Value: "bar",
		},
	}

	testQemuKernelParameters(t, params, expectedOut, true)
	testQemuKernelParameters(t, params, expectedOut, false)
}

func TestQemuCreateVM(t *testing.T) {
	assert := assert.New(t)

	store, err := persist.GetDriver()
	assert.NoError(err)

	// Create the hypervisor fake binary
	testQemuPath := filepath.Join(testDir, testHypervisor)
	_, err = os.Create(testQemuPath)
	assert.NoError(err)

	// Create parent dir path for hypervisor.json
	parentDir := filepath.Join(store.RunStoragePath(), "testSandbox")
	assert.NoError(os.MkdirAll(parentDir, DirMode))

	network, err := NewNetwork()
	assert.NoError(err)

	config0 := newQemuConfig()

	config1 := newQemuConfig()
	config1.SeccompSandbox = "enable=1"

	config2 := newQemuConfig()
	config2.InitrdPath = ""
	config2.ImagePath = testQemuImagePath

	config3 := newQemuConfig()
	config3.Debug = true

	config5 := newQemuConfig()
	config5.GuestMemoryDumpPath = "/tmp"

	config6 := newQemuConfig()
	config6.DisableGuestSeLinux = false

	config8 := newQemuConfig()
	config8.EnableVhostUserStore = true
	config8.HugePages = true

	config9 := newQemuConfig()
	config9.EnableVhostUserStore = true
	config9.HugePages = false

	config10 := newQemuConfig()
	config10.BootToBeTemplate = true

	config11 := newQemuConfig()
	config11.BootFromTemplate = true

	config12 := newQemuConfig()
	config12.BootToBeTemplate = true
	config12.SharedFS = config.VirtioFS

	config13 := newQemuConfig()
	config13.FileBackedMemRootDir = "/tmp/xyzabc"
	config13.HugePages = true

	config14 := newQemuConfig()
	config14.SharedFS = config.VirtioFS

	config15 := newQemuConfig()
	config15.BlockDeviceDriver = ""

	config16 := newQemuConfig()
	config16.SharedFS = config.VirtioFSNydus

	config17 := newQemuConfig()
	config17.VMid = "testSandbox"

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
		{config5, false, true},
		{config6, false, false},
		{config8, false, true},
		{config9, true, false},
		{config10, false, true},
		{config11, false, true},
		{config12, true, false},
		{config13, false, true},
		{config14, false, true},
		{config15, false, true},
		{config16, false, true},
		{config17, false, true},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]", i)

		q := &qemu{
			config: HypervisorConfig{
				VMStorePath:  store.RunVMStoragePath(),
				RunStorePath: store.RunStoragePath(),
			},
		}

		err = q.CreateVM(context.Background(), "testSandbox", network, &d.config)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)

		if d.configMatch {
			assert.Exactly(d.config, q.config, msg)
		}

		mem := q.GetTotalMemoryMB(context.Background())
		assert.True(mem > 0)

		err = q.canDumpGuestMemory("/tmp")
		assert.NoError(err)

		err = q.dumpGuestMemory("")
		assert.NoError(err)

		q.dumpSandboxMetaInfo("/tmp/")

		// now we exercise code that should fail since the VM isn't running
		err = q.dumpGuestMemory("/tmp")
		assert.Error(err)

		err = q.setupVirtioMem(context.Background())
		assert.Error(err)

		err = q.SaveVM()
		assert.Error(err)

		err = q.StopVM(context.Background(), true)
		assert.Error(err)
	}

	assert.NoError(os.RemoveAll(parentDir))
}

func TestQemuCreateVMMissingParentDirFail(t *testing.T) {
	qemuConfig := newQemuConfig()
	assert := assert.New(t)

	store, err := persist.GetDriver()
	assert.NoError(err)
	q := &qemu{
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
	}
	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "testSandbox",
		config: &SandboxConfig{
			HypervisorConfig: qemuConfig,
		},
	}

	// Create the hypervisor fake binary
	testQemuPath := filepath.Join(testDir, testHypervisor)
	_, err = os.Create(testQemuPath)
	assert.NoError(err)

	// Ensure parent dir path for hypervisor.json does not exist.
	parentDir := filepath.Join(store.RunStoragePath(), sandbox.id)
	assert.NoError(os.RemoveAll(parentDir))

	network, err := NewNetwork()
	assert.NoError(err)
	err = q.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)
	assert.NoError(err)
}

func TestQemuCPUTopology(t *testing.T) {
	assert := assert.New(t)
	vcpus := float32(1)

	q := &qemu{
		arch: &qemuArchBase{},
		config: HypervisorConfig{
			NumVCPUsF:       vcpus,
			DefaultMaxVCPUs: uint32(vcpus),
		},
	}

	expectedOut := govmmQemu.SMP{
		CPUs:    uint32(vcpus),
		Sockets: uint32(vcpus),
		Cores:   defaultCores,
		Threads: defaultThreads,
		MaxCPUs: uint32(vcpus),
	}

	smp := q.cpuTopology()
	assert.Exactly(smp, expectedOut)
}

func TestQemuMemoryTopology(t *testing.T) {
	mem := uint32(1000)
	maxMem := memory.TotalMemory() / 1024 / 1024 //MiB
	slots := uint32(8)
	assert := assert.New(t)

	q := &qemu{
		arch: &qemuArchBase{},
		config: HypervisorConfig{
			MemorySize:           mem,
			MemSlots:             slots,
			DefaultMaxMemorySize: maxMem,
		},
	}

	memMax := fmt.Sprintf("%dM", int(maxMem))

	expectedOut := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  uint8(slots),
		MaxMem: memMax,
	}

	memory, err := q.memoryTopology()
	assert.NoError(err)
	assert.Exactly(memory, expectedOut)
}

func TestQemuKnobs(t *testing.T) {
	assert := assert.New(t)

	sandbox, err := createQemuSandboxConfig()
	assert.NoError(err)

	q := &qemu{
		config: HypervisorConfig{
			VMStorePath:  sandbox.store.RunVMStoragePath(),
			RunStorePath: sandbox.store.RunStoragePath(),
		},
	}
	network, err := NewNetwork()
	assert.NoError(err)
	err = q.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)
	assert.NoError(err)

	assert.Equal(q.qemuConfig.Knobs.NoUserConfig, true)
	assert.Equal(q.qemuConfig.Knobs.NoDefaults, true)
	assert.Equal(q.qemuConfig.Knobs.NoGraphic, true)
	assert.Equal(q.qemuConfig.Knobs.NoReboot, true)
}

func testQemuAddDevice(t *testing.T, devInfo interface{}, devType DeviceType, expected []govmmQemu.Device) {
	assert := assert.New(t)
	q := &qemu{
		ctx:  context.Background(),
		arch: &qemuArchBase{},
	}

	err := q.AddDevice(context.Background(), devInfo, devType)
	assert.NoError(err)
	assert.Exactly(q.qemuConfig.Devices, expected)
}

func TestQemuAddDeviceFsDev(t *testing.T) {
	mountTag := "testMountTag"
	hostPath := "testHostPath"

	expectedOut := []govmmQemu.Device{
		govmmQemu.FSDevice{
			Driver:        govmmQemu.Virtio9P,
			FSDriver:      govmmQemu.Local,
			ID:            fmt.Sprintf("extra-9p-%s", mountTag),
			Path:          hostPath,
			MountTag:      mountTag,
			SecurityModel: govmmQemu.None,
			Multidev:      govmmQemu.Remap,
		},
	}

	volume := types.Volume{
		MountTag: mountTag,
		HostPath: hostPath,
	}

	testQemuAddDevice(t, volume, FsDev, expectedOut)
}

func TestQemuAddDeviceVhostUserBlk(t *testing.T) {
	socketPath := "/test/socket/path"
	devID := "testDevID"

	expectedOut := []govmmQemu.Device{
		govmmQemu.VhostUserDevice{
			SocketPath:    socketPath,
			CharDevID:     utils.MakeNameID("char", devID, maxDevIDSize),
			VhostUserType: govmmQemu.VhostUserBlk,
		},
	}

	vDevice := config.VhostUserDeviceAttrs{
		DevID:      devID,
		SocketPath: socketPath,
		Type:       config.VhostUserBlk,
	}

	testQemuAddDevice(t, vDevice, VhostuserDev, expectedOut)
}

func TestQemuAddDeviceSerialPortDev(t *testing.T) {
	deviceID := "channelTest"
	id := "charchTest"
	hostPath := "/tmp/hyper_test.sock"
	name := "sh.hyper.channel.test"

	expectedOut := []govmmQemu.Device{
		govmmQemu.CharDevice{
			Driver:   govmmQemu.VirtioSerialPort,
			Backend:  govmmQemu.Socket,
			DeviceID: deviceID,
			ID:       id,
			Path:     hostPath,
			Name:     name,
		},
	}

	socket := types.Socket{
		DeviceID: deviceID,
		ID:       id,
		HostPath: hostPath,
		Name:     name,
	}

	testQemuAddDevice(t, socket, SerialPortDev, expectedOut)
}

func TestQemuAddDeviceKataVSOCK(t *testing.T) {
	assert := assert.New(t)

	dir := t.TempDir()

	vsockFilename := filepath.Join(dir, "vsock")

	contextID := uint64(3)
	port := uint32(1024)

	vsockFile, err := os.Create(vsockFilename)
	assert.NoError(err)
	defer vsockFile.Close()

	expectedOut := []govmmQemu.Device{
		govmmQemu.VSOCKDevice{
			ID:        fmt.Sprintf("vsock-%d", contextID),
			ContextID: contextID,
			VHostFD:   vsockFile,
		},
	}

	vsock := types.VSock{
		ContextID: contextID,
		Port:      port,
		VhostFd:   vsockFile,
	}

	testQemuAddDevice(t, vsock, VSockPCIDev, expectedOut)
}

func TestQemuGetSandboxConsole(t *testing.T) {
	assert := assert.New(t)
	store, err := persist.GetDriver()
	assert.NoError(err)
	q := &qemu{
		ctx: context.Background(),
		config: HypervisorConfig{
			VMStorePath:  store.RunVMStoragePath(),
			RunStorePath: store.RunStoragePath(),
		},
	}
	sandboxID := "testSandboxID"
	expected := filepath.Join(store.RunVMStoragePath(), sandboxID, consoleSocket)

	proto, result, err := q.GetVMConsole(q.ctx, sandboxID)
	assert.NoError(err)
	assert.Equal(result, expected)
	assert.Equal(proto, consoleProtoUnix)
}

func TestQemuCapabilities(t *testing.T) {
	assert := assert.New(t)
	q := &qemu{
		ctx:  context.Background(),
		arch: &qemuArchBase{},
	}

	caps := q.Capabilities(q.ctx)
	assert.True(caps.IsBlockDeviceHotplugSupported())
	assert.True(caps.IsNetworkDeviceHotplugSupported())
}

func TestQemuQemuPath(t *testing.T) {
	assert := assert.New(t)

	f, err := os.CreateTemp("", "qemu")
	assert.NoError(err)
	defer func() { _ = f.Close() }()
	defer func() { _ = os.Remove(f.Name()) }()

	expectedPath := f.Name()
	qemuConfig := newQemuConfig()
	qemuConfig.HypervisorPath = expectedPath
	qkvm := &qemuArchBase{
		qemuMachine: govmmQemu.Machine{
			Type:    "q35",
			Options: "",
		},
		qemuExePath: expectedPath,
	}

	q := &qemu{
		config: qemuConfig,
		arch:   qkvm,
	}

	// get config hypervisor path
	path, err := q.qemuPath()
	assert.NoError(err)
	assert.Equal(path, expectedPath)

	// config hypervisor path does not exist
	q.config.HypervisorPath = "/abc/rgb/123"
	path, err = q.qemuPath()
	assert.Error(err)
	assert.Equal(path, "")

	// get arch hypervisor path
	q.config.HypervisorPath = ""
	path, err = q.qemuPath()
	assert.NoError(err)
	assert.Equal(path, expectedPath)
}

func TestHotplugUnsupportedDeviceType(t *testing.T) {
	assert := assert.New(t)

	qemuConfig := newQemuConfig()
	q := &qemu{
		ctx:    context.Background(),
		id:     "qemuTest",
		config: qemuConfig,
	}

	_, err := q.HotplugAddDevice(context.Background(), &MemoryDevice{0, 128, uint64(0), false}, FsDev)
	assert.Error(err)
	_, err = q.HotplugRemoveDevice(context.Background(), &MemoryDevice{0, 128, uint64(0), false}, FsDev)
	assert.Error(err)
}

func TestQMPSetupShutdown(t *testing.T) {
	assert := assert.New(t)

	qemuConfig := newQemuConfig()
	q := &qemu{
		config: qemuConfig,
	}

	q.qmpShutdown()

	q.qmpMonitorCh.qmp = &govmmQemu.QMP{}
	err := q.qmpSetup()
	assert.Nil(err)
}

func TestQemuCleanup(t *testing.T) {
	assert := assert.New(t)

	q := &qemu{
		ctx:    context.Background(),
		config: newQemuConfig(),
	}

	err := q.Cleanup(q.ctx)
	assert.Nil(err)
}

func TestQemuGrpc(t *testing.T) {
	assert := assert.New(t)

	config := newQemuConfig()
	q := &qemu{
		id:     "testqemu",
		config: config,
	}

	json, err := q.toGrpc(context.Background())
	assert.Nil(err)

	var q2 qemu
	err = q2.fromGrpc(context.Background(), &config, json)
	assert.Nil(err)

	assert.True(q.id == q2.id)
}

func TestQemuFileBackedMem(t *testing.T) {
	assert := assert.New(t)

	// Check default Filebackedmem location for virtio-fs
	sandbox, err := createQemuSandboxConfig()
	assert.NoError(err)

	network, err := NewNetwork()
	assert.NoError(err)

	q := &qemu{
		config: HypervisorConfig{
			VMStorePath:  sandbox.store.RunVMStoragePath(),
			RunStorePath: sandbox.store.RunStoragePath(),
		},
	}
	sandbox.config.HypervisorConfig.SharedFS = config.VirtioFS
	err = q.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)
	assert.NoError(err)

	assert.Equal(q.qemuConfig.Knobs.FileBackedMem, true)
	assert.Equal(q.qemuConfig.Knobs.MemShared, true)
	assert.Equal(q.qemuConfig.Memory.Path, fallbackFileBackedMemDir)

	// Check failure for VM templating
	sandbox, err = createQemuSandboxConfig()
	assert.NoError(err)

	q = &qemu{
		config: HypervisorConfig{
			VMStorePath:  sandbox.store.RunVMStoragePath(),
			RunStorePath: sandbox.store.RunStoragePath(),
		},
	}
	sandbox.config.HypervisorConfig.BootToBeTemplate = true
	sandbox.config.HypervisorConfig.SharedFS = config.VirtioFS
	sandbox.config.HypervisorConfig.MemoryPath = fallbackFileBackedMemDir

	err = q.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)

	expectErr := errors.New("VM templating has been enabled with either virtio-fs or file backed memory and this configuration will not work")
	assert.Equal(expectErr.Error(), err.Error())

	// Check Setting of non-existent shared-mem path
	sandbox, err = createQemuSandboxConfig()
	assert.NoError(err)

	q = &qemu{
		config: HypervisorConfig{
			VMStorePath:  sandbox.store.RunVMStoragePath(),
			RunStorePath: sandbox.store.RunStoragePath(),
		},
	}
	sandbox.config.HypervisorConfig.FileBackedMemRootDir = "/tmp/xyzabc"
	err = q.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)
	assert.NoError(err)
	assert.Equal(q.qemuConfig.Knobs.FileBackedMem, false)
	assert.Equal(q.qemuConfig.Knobs.MemShared, false)
	assert.Equal(q.qemuConfig.Memory.Path, "")

	// Check setting vhost-user storage with Hugepages
	sandbox, err = createQemuSandboxConfig()
	assert.NoError(err)

	q = &qemu{
		config: HypervisorConfig{
			VMStorePath:  sandbox.store.RunVMStoragePath(),
			RunStorePath: sandbox.store.RunStoragePath(),
		},
	}
	sandbox.config.HypervisorConfig.EnableVhostUserStore = true
	sandbox.config.HypervisorConfig.HugePages = true
	err = q.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)
	assert.NoError(err)
	assert.Equal(q.qemuConfig.Knobs.MemShared, true)

	// Check failure for vhost-user storage
	sandbox, err = createQemuSandboxConfig()
	assert.NoError(err)

	q = &qemu{
		config: HypervisorConfig{
			VMStorePath:  sandbox.store.RunVMStoragePath(),
			RunStorePath: sandbox.store.RunStoragePath(),
		},
	}
	sandbox.config.HypervisorConfig.EnableVhostUserStore = true
	sandbox.config.HypervisorConfig.HugePages = false
	err = q.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)

	expectErr = errors.New("Vhost-user-blk/scsi is enabled without HugePages. This configuration will not work")
	assert.Equal(expectErr.Error(), err.Error())
}

func createQemuSandboxConfig() (*Sandbox, error) {

	qemuConfig := newQemuConfig()
	sandbox := Sandbox{
		ctx: context.Background(),
		id:  "testSandbox",
		config: &SandboxConfig{
			HypervisorConfig: qemuConfig,
		},
	}

	store, err := persist.GetDriver()
	if err != nil {
		return &Sandbox{}, err
	}
	sandbox.store = store

	return &sandbox, nil
}

func TestQemuGetpids(t *testing.T) {
	assert := assert.New(t)

	qemuConfig := newQemuConfig()
	q := &qemu{}
	pids := q.GetPids()
	assert.NotNil(pids)
	assert.True(len(pids) == 1)
	assert.True(pids[0] == 0)

	q = &qemu{
		config: qemuConfig,
	}
	f, err := os.CreateTemp("", "qemu-test-")
	assert.Nil(err)
	tmpfile := f.Name()
	f.Close()
	defer os.Remove(tmpfile)

	q.qemuConfig.PidFile = tmpfile
	pids = q.GetPids()
	assert.True(len(pids) == 1)
	assert.True(pids[0] == 0)

	err = os.WriteFile(tmpfile, []byte("100"), 0)
	assert.Nil(err)
	pids = q.GetPids()
	assert.True(len(pids) == 1)
	assert.True(pids[0] == 100)

	q.state.VirtiofsDaemonPid = 200
	pids = q.GetPids()
	assert.True(len(pids) == 2)
	assert.True(pids[0] == 100)
	assert.True(pids[1] == 200)
}

func TestQemuSetConfig(t *testing.T) {
	assert := assert.New(t)

	config := newQemuConfig()

	q := &qemu{}

	assert.Equal(q.config, HypervisorConfig{})
	err := q.setConfig(&config)
	assert.NoError(err)

	assert.Equal(q.config, config)
}

func TestQemuStartSandbox(t *testing.T) {
	assert := assert.New(t)

	sandbox, err := createQemuSandboxConfig()
	assert.NoError(err)

	network, err := NewNetwork()
	assert.NoError(err)

	q := &qemu{
		config: HypervisorConfig{
			VMStorePath:  sandbox.store.RunVMStoragePath(),
			RunStorePath: sandbox.store.RunStoragePath(),
		},
		virtiofsDaemon: &virtiofsdMock{},
	}

	err = q.CreateVM(context.Background(), sandbox.id, network, &sandbox.config.HypervisorConfig)
	assert.NoError(err)

	err = q.StartVM(context.Background(), 10)
	assert.Error(err)
}
