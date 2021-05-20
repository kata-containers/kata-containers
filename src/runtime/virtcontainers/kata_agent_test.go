// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"reflect"
	"strings"
	"syscall"
	"testing"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/manager"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	pb "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var (
	testBlkDriveFormat     = "testBlkDriveFormat"
	testBlockDeviceCtrPath = "testBlockDeviceCtrPath"
	testDevNo              = "testDevNo"
	testNvdimmID           = "testNvdimmID"
	testPCIPath, _         = vcTypes.PciPathFromString("04/02")
	testSCSIAddr           = "testSCSIAddr"
	testVirtPath           = "testVirtPath"
)

func TestKataAgentConnect(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	err = k.connect(context.Background())
	assert.NoError(err)
	assert.NotNil(k.client)
}

func TestKataAgentDisconnect(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	assert.NoError(k.connect(context.Background()))
	assert.NoError(k.disconnect(context.Background()))
	assert.Nil(k.client)
}

var reqList = []interface{}{
	&pb.CreateSandboxRequest{},
	&pb.DestroySandboxRequest{},
	&pb.ExecProcessRequest{},
	&pb.CreateContainerRequest{},
	&pb.StartContainerRequest{},
	&pb.RemoveContainerRequest{},
	&pb.SignalProcessRequest{},
	&pb.CheckRequest{},
	&pb.WaitProcessRequest{},
	&pb.StatsContainerRequest{},
	&pb.SetGuestDateTimeRequest{},
}

func TestKataAgentSendReq(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	ctx := context.Background()

	for _, req := range reqList {
		_, err = k.sendReq(ctx, req)
		assert.Nil(err)
	}

	sandbox := &Sandbox{}
	container := &Container{}
	execid := "processFooBar"

	err = k.startContainer(ctx, sandbox, container)
	assert.Nil(err)

	err = k.signalProcess(ctx, container, execid, syscall.SIGKILL, true)
	assert.Nil(err)

	err = k.winsizeProcess(ctx, container, execid, 100, 200)
	assert.Nil(err)

	err = k.updateContainer(ctx, sandbox, Container{}, specs.LinuxResources{})
	assert.Nil(err)

	err = k.pauseContainer(ctx, sandbox, Container{})
	assert.Nil(err)

	err = k.resumeContainer(ctx, sandbox, Container{})
	assert.Nil(err)

	err = k.onlineCPUMem(ctx, 1, true)
	assert.Nil(err)

	_, err = k.statsContainer(ctx, sandbox, Container{})
	assert.Nil(err)

	err = k.check(ctx)
	assert.Nil(err)

	_, err = k.waitProcess(ctx, container, execid)
	assert.Nil(err)

	_, err = k.writeProcessStdin(ctx, container, execid, []byte{'c'})
	assert.Nil(err)

	err = k.closeProcessStdin(ctx, container, execid)
	assert.Nil(err)

	_, err = k.readProcessStdout(ctx, container, execid, []byte{})
	assert.Nil(err)

	_, err = k.readProcessStderr(ctx, container, execid, []byte{})
	assert.Nil(err)

	_, err = k.getOOMEvent(ctx)
	assert.Nil(err)
}

func TestHandleEphemeralStorage(t *testing.T) {
	k := kataAgent{}
	var ociMounts []specs.Mount
	mountSource := "/tmp/mountPoint"
	os.Mkdir(mountSource, 0755)

	mount := specs.Mount{
		Type:   KataEphemeralDevType,
		Source: mountSource,
	}

	ociMounts = append(ociMounts, mount)
	epheStorages, err := k.handleEphemeralStorage(ociMounts)
	assert.Nil(t, err)

	epheMountPoint := epheStorages[0].MountPoint
	expected := filepath.Join(ephemeralPath(), filepath.Base(mountSource))
	assert.Equal(t, epheMountPoint, expected,
		"Ephemeral mount point didn't match: got %s, expecting %s", epheMountPoint, expected)
}

func TestHandleLocalStorage(t *testing.T) {
	k := kataAgent{}
	var ociMounts []specs.Mount
	mountSource := "/tmp/mountPoint"
	os.Mkdir(mountSource, 0755)

	mount := specs.Mount{
		Type:   KataLocalDevType,
		Source: mountSource,
	}

	sandboxID := "sandboxid"
	rootfsSuffix := "rootfs"

	ociMounts = append(ociMounts, mount)
	localStorages, _ := k.handleLocalStorage(ociMounts, sandboxID, rootfsSuffix)

	assert.NotNil(t, localStorages)
	assert.Equal(t, len(localStorages), 1)

	localMountPoint := localStorages[0].MountPoint
	expected := filepath.Join(kataGuestSharedDir(), sandboxID, rootfsSuffix, KataLocalDevType, filepath.Base(mountSource))
	assert.Equal(t, localMountPoint, expected)
}

func TestHandleDeviceBlockVolume(t *testing.T) {
	k := kataAgent{}

	tests := []struct {
		BlockDeviceDriver string
		inputMount        Mount
		inputDev          *drivers.BlockDevice
		resultVol         *pb.Storage
	}{
		{
			inputDev: &drivers.BlockDevice{
				BlockDrive: &config.BlockDrive{
					Pmem:     true,
					NvdimmID: testNvdimmID,
					Format:   testBlkDriveFormat,
				},
			},
			inputMount: Mount{},
			resultVol: &pb.Storage{
				Driver:  kataNvdimmDevType,
				Source:  fmt.Sprintf("/dev/pmem%s", testNvdimmID),
				Fstype:  testBlkDriveFormat,
				Options: []string{"dax"},
			},
		},
		{
			BlockDeviceDriver: config.VirtioBlockCCW,
			inputMount: Mount{
				Type:    "bind",
				Options: []string{"ro"},
			},
			inputDev: &drivers.BlockDevice{
				BlockDrive: &config.BlockDrive{
					DevNo: testDevNo,
				},
			},
			resultVol: &pb.Storage{
				Driver:  kataBlkCCWDevType,
				Source:  testDevNo,
				Fstype:  "bind",
				Options: []string{"ro"},
			},
		},
		{
			BlockDeviceDriver: config.VirtioBlock,
			inputMount:        Mount{},
			inputDev: &drivers.BlockDevice{
				BlockDrive: &config.BlockDrive{
					PCIPath:  testPCIPath,
					VirtPath: testVirtPath,
				},
			},
			resultVol: &pb.Storage{
				Driver: kataBlkDevType,
				Source: testPCIPath.String(),
			},
		},
		{
			BlockDeviceDriver: config.VirtioMmio,
			inputDev: &drivers.BlockDevice{
				BlockDrive: &config.BlockDrive{
					VirtPath: testVirtPath,
				},
			},
			resultVol: &pb.Storage{
				Driver: kataMmioBlkDevType,
				Source: testVirtPath,
			},
		},
		{
			BlockDeviceDriver: config.VirtioSCSI,
			inputDev: &drivers.BlockDevice{
				BlockDrive: &config.BlockDrive{
					SCSIAddr: testSCSIAddr,
				},
			},
			resultVol: &pb.Storage{
				Driver: kataSCSIDevType,
				Source: testSCSIAddr,
			},
		},
	}

	for _, test := range tests {
		c := &Container{
			sandbox: &Sandbox{
				config: &SandboxConfig{
					HypervisorConfig: HypervisorConfig{
						BlockDeviceDriver: test.BlockDeviceDriver,
					},
				},
			},
		}

		vol, _ := k.handleDeviceBlockVolume(c, test.inputMount, test.inputDev)
		assert.True(t, reflect.DeepEqual(vol, test.resultVol),
			"Volume didn't match: got %+v, expecting %+v",
			vol, test.resultVol)
	}
}

func TestHandleBlockVolume(t *testing.T) {
	k := kataAgent{}

	c := &Container{
		id: "100",
	}
	containers := map[string]*Container{}
	containers[c.id] = c

	// Create a devices for VhostUserBlk, standard DeviceBlock and direct assigned Block device
	vDevID := "MockVhostUserBlk"
	bDevID := "MockDeviceBlock"
	dDevID := "MockDeviceBlockDirect"
	vDestination := "/VhostUserBlk/destination"
	bDestination := "/DeviceBlock/destination"
	dDestination := "/DeviceDirectBlock/destination"
	vPCIPath, err := vcTypes.PciPathFromString("01/02")
	assert.NoError(t, err)
	bPCIPath, err := vcTypes.PciPathFromString("03/04")
	assert.NoError(t, err)
	dPCIPath, err := vcTypes.PciPathFromString("04/05")
	assert.NoError(t, err)

	vDev := drivers.NewVhostUserBlkDevice(&config.DeviceInfo{ID: vDevID})
	bDev := drivers.NewBlockDevice(&config.DeviceInfo{ID: bDevID})
	dDev := drivers.NewBlockDevice(&config.DeviceInfo{ID: dDevID})

	vDev.VhostUserDeviceAttrs = &config.VhostUserDeviceAttrs{PCIPath: vPCIPath}
	bDev.BlockDrive = &config.BlockDrive{PCIPath: bPCIPath}
	dDev.BlockDrive = &config.BlockDrive{PCIPath: dPCIPath}

	var devices []api.Device
	devices = append(devices, vDev, bDev, dDev)

	// Create a VhostUserBlk mount and a DeviceBlock mount
	var mounts []Mount
	vMount := Mount{
		BlockDeviceID: vDevID,
		Destination:   vDestination,
	}
	bMount := Mount{
		BlockDeviceID: bDevID,
		Destination:   bDestination,
		Type:          "bind",
		Options:       []string{"bind"},
	}
	dMount := Mount{
		BlockDeviceID: dDevID,
		Destination:   dDestination,
		Type:          "ext4",
		Options:       []string{"ro"},
	}
	mounts = append(mounts, vMount, bMount, dMount)

	tmpDir := "/vhost/user/dir"
	dm := manager.NewDeviceManager(manager.VirtioBlock, true, tmpDir, devices)

	sConfig := SandboxConfig{}
	sConfig.HypervisorConfig.BlockDeviceDriver = manager.VirtioBlock
	sandbox := Sandbox{
		id:         "100",
		containers: containers,
		hypervisor: &mockHypervisor{},
		devManager: dm,
		ctx:        context.Background(),
		config:     &sConfig,
	}
	containers[c.id].sandbox = &sandbox
	containers[c.id].mounts = mounts

	volumeStorages, err := k.handleBlockVolumes(c)
	assert.Nil(t, err, "Error while handling block volumes")

	vStorage := &pb.Storage{
		MountPoint: vDestination,
		Fstype:     "bind",
		Options:    []string{"bind"},
		Driver:     kataBlkDevType,
		Source:     vPCIPath.String(),
	}
	bStorage := &pb.Storage{
		MountPoint: bDestination,
		Fstype:     "bind",
		Options:    []string{"bind"},
		Driver:     kataBlkDevType,
		Source:     bPCIPath.String(),
	}
	dStorage := &pb.Storage{
		MountPoint: dDestination,
		Fstype:     "ext4",
		Options:    []string{"ro"},
		Driver:     kataBlkDevType,
		Source:     dPCIPath.String(),
	}

	assert.Equal(t, vStorage, volumeStorages[0], "Error while handle VhostUserBlk type block volume")
	assert.Equal(t, bStorage, volumeStorages[1], "Error while handle BlockDevice type block volume")
	assert.Equal(t, dStorage, volumeStorages[2], "Error while handle direct BlockDevice type block volume")
}

func TestAppendDevicesEmptyContainerDeviceList(t *testing.T) {
	k := kataAgent{}

	devList := []*pb.Device{}
	expected := []*pb.Device{}
	ctrDevices := []ContainerDevice{}

	c := &Container{
		sandbox: &Sandbox{
			devManager: manager.NewDeviceManager("virtio-scsi", false, "", nil),
		},
		devices: ctrDevices,
	}
	updatedDevList := k.appendDevices(devList, c)
	assert.True(t, reflect.DeepEqual(updatedDevList, expected),
		"Device lists didn't match: got %+v, expecting %+v",
		updatedDevList, expected)
}

func TestAppendDevices(t *testing.T) {
	k := kataAgent{}

	id := "test-append-block"
	ctrDevices := []api.Device{
		&drivers.BlockDevice{
			GenericDevice: &drivers.GenericDevice{
				ID: id,
			},
			BlockDrive: &config.BlockDrive{
				PCIPath: testPCIPath,
			},
		},
	}

	sandboxConfig := &SandboxConfig{
		HypervisorConfig: HypervisorConfig{
			BlockDeviceDriver: config.VirtioBlock,
		},
	}

	c := &Container{
		sandbox: &Sandbox{
			devManager: manager.NewDeviceManager("virtio-blk", false, "", ctrDevices),
			config:     sandboxConfig,
		},
	}
	c.devices = append(c.devices, ContainerDevice{
		ID:            id,
		ContainerPath: testBlockDeviceCtrPath,
	})

	devList := []*pb.Device{}
	expected := []*pb.Device{
		{
			Type:          kataBlkDevType,
			ContainerPath: testBlockDeviceCtrPath,
			Id:            testPCIPath.String(),
		},
	}
	updatedDevList := k.appendDevices(devList, c)
	assert.True(t, reflect.DeepEqual(updatedDevList, expected),
		"Device lists didn't match: got %+v, expecting %+v",
		updatedDevList, expected)
}

func TestAppendVhostUserBlkDevices(t *testing.T) {
	k := kataAgent{}

	id := "test-append-vhost-user-blk"
	ctrDevices := []api.Device{
		&drivers.VhostUserBlkDevice{
			GenericDevice: &drivers.GenericDevice{
				ID: id,
			},
			VhostUserDeviceAttrs: &config.VhostUserDeviceAttrs{
				Type:    config.VhostUserBlk,
				PCIPath: testPCIPath,
			},
		},
	}

	sandboxConfig := &SandboxConfig{
		HypervisorConfig: HypervisorConfig{
			BlockDeviceDriver: config.VirtioBlock,
		},
	}

	testVhostUserStorePath := "/test/vhost/user/store/path"
	c := &Container{
		sandbox: &Sandbox{
			devManager: manager.NewDeviceManager("virtio-blk", true, testVhostUserStorePath, ctrDevices),
			config:     sandboxConfig,
		},
	}
	c.devices = append(c.devices, ContainerDevice{
		ID:            id,
		ContainerPath: testBlockDeviceCtrPath,
	})

	devList := []*pb.Device{}
	expected := []*pb.Device{
		{
			Type:          kataBlkDevType,
			ContainerPath: testBlockDeviceCtrPath,
			Id:            testPCIPath.String(),
		},
	}
	updatedDevList := k.appendDevices(devList, c)
	assert.True(t, reflect.DeepEqual(updatedDevList, expected),
		"Device lists didn't match: got %+v, expecting %+v",
		updatedDevList, expected)
}

func TestConstraintGRPCSpec(t *testing.T) {
	assert := assert.New(t)
	expectedCgroupPath := "/foo/bar"

	g := &pb.Spec{
		Hooks: &pb.Hooks{},
		Mounts: []pb.Mount{
			{Destination: "/dev/shm"},
		},
		Linux: &pb.Linux{
			Seccomp: &pb.LinuxSeccomp{},
			Namespaces: []pb.LinuxNamespace{
				{
					Type: specs.NetworkNamespace,
					Path: "/abc/123",
				},
				{
					Type: specs.MountNamespace,
					Path: "/abc/123",
				},
			},
			Resources: &pb.LinuxResources{
				Devices:        []pb.LinuxDeviceCgroup{},
				Memory:         &pb.LinuxMemory{},
				CPU:            &pb.LinuxCPU{},
				Pids:           &pb.LinuxPids{},
				BlockIO:        &pb.LinuxBlockIO{},
				HugepageLimits: []pb.LinuxHugepageLimit{},
				Network:        &pb.LinuxNetwork{},
			},
			CgroupsPath: "system.slice:foo:bar",
			Devices: []pb.LinuxDevice{
				{
					Path: "/dev/vfio/1",
					Type: "c",
				},
				{
					Path: "/dev/vfio/2",
					Type: "c",
				},
			},
		},
		Process: &pb.Process{
			SelinuxLabel: "foo",
		},
	}

	k := kataAgent{}
	k.constraintGRPCSpec(g, true)

	// check nil fields
	assert.Nil(g.Hooks)
	assert.NotNil(g.Linux.Seccomp)
	assert.Nil(g.Linux.Resources.Devices)
	assert.NotNil(g.Linux.Resources.Memory)
	assert.Nil(g.Linux.Resources.Pids)
	assert.Nil(g.Linux.Resources.BlockIO)
	assert.Nil(g.Linux.Resources.HugepageLimits)
	assert.Nil(g.Linux.Resources.Network)
	assert.NotNil(g.Linux.Resources.CPU)
	assert.Equal(g.Process.SelinuxLabel, "")

	// check namespaces
	assert.Len(g.Linux.Namespaces, 1)
	assert.Empty(g.Linux.Namespaces[0].Path)

	// check mounts
	assert.Len(g.Mounts, 1)

	// check cgroup path
	assert.Equal(expectedCgroupPath, g.Linux.CgroupsPath)

	// check Linux devices
	assert.Empty(g.Linux.Devices)
}

func TestHandleShm(t *testing.T) {
	assert := assert.New(t)
	k := kataAgent{}
	sandbox := &Sandbox{
		shmSize: 8192,
	}

	var ociMounts []specs.Mount

	mount := specs.Mount{
		Type:        "bind",
		Destination: "/dev/shm",
	}

	ociMounts = append(ociMounts, mount)
	k.handleShm(ociMounts, sandbox)

	assert.Len(ociMounts, 1)
	assert.NotEmpty(ociMounts[0].Destination)
	assert.Equal(ociMounts[0].Destination, "/dev/shm")
	assert.Equal(ociMounts[0].Type, "bind")
	assert.NotEmpty(ociMounts[0].Source, filepath.Join(kataGuestSharedDir(), shmDir))
	assert.Equal(ociMounts[0].Options, []string{"rbind"})

	sandbox.shmSize = 0
	k.handleShm(ociMounts, sandbox)

	assert.Len(ociMounts, 1)
	assert.Equal(ociMounts[0].Destination, "/dev/shm")
	assert.Equal(ociMounts[0].Type, "tmpfs")
	assert.Equal(ociMounts[0].Source, "shm")
	sizeOption := fmt.Sprintf("size=%d", DefaultShmSize)
	assert.Equal(ociMounts[0].Options, []string{"noexec", "nosuid", "nodev", "mode=1777", sizeOption})

	// In case the type of mount is ephemeral, the container mount is not
	// shared with the sandbox shm.
	ociMounts[0].Type = KataEphemeralDevType
	mountSource := "/tmp/mountPoint"
	os.Mkdir(mountSource, 0755)
	ociMounts[0].Source = mountSource
	k.handleShm(ociMounts, sandbox)

	assert.Len(ociMounts, 1)
	assert.Equal(ociMounts[0].Type, KataEphemeralDevType)
	assert.NotEmpty(ociMounts[0].Source, mountSource)

	epheStorages, err := k.handleEphemeralStorage(ociMounts)
	assert.Nil(err)

	epheMountPoint := epheStorages[0].MountPoint
	expected := filepath.Join(ephemeralPath(), filepath.Base(mountSource))
	assert.Equal(epheMountPoint, expected,
		"Ephemeral mount point didn't match: got %s, expecting %s", epheMountPoint, expected)

}

func testIsPidNamespacePresent(grpcSpec *pb.Spec) bool {
	for _, ns := range grpcSpec.Linux.Namespaces {
		if ns.Type == string(specs.PIDNamespace) {
			return true
		}
	}

	return false
}

func TestHandlePidNamespace(t *testing.T) {
	assert := assert.New(t)

	g := &pb.Spec{
		Linux: &pb.Linux{
			Namespaces: []pb.LinuxNamespace{
				{
					Type: specs.NetworkNamespace,
					Path: "/abc/123",
				},
				{
					Type: specs.MountNamespace,
					Path: "/abc/123",
				},
			},
		},
	}

	sandbox := &Sandbox{}

	k := kataAgent{}

	sharedPid := k.handlePidNamespace(g, sandbox)
	assert.False(sharedPid)
	assert.False(testIsPidNamespacePresent(g))

	pidNs := pb.LinuxNamespace{
		Type: string(specs.PIDNamespace),
		Path: "",
	}

	utsNs := pb.LinuxNamespace{
		Type: specs.UTSNamespace,
		Path: "",
	}

	g.Linux.Namespaces = append(g.Linux.Namespaces, pidNs)
	g.Linux.Namespaces = append(g.Linux.Namespaces, utsNs)

	sharedPid = k.handlePidNamespace(g, sandbox)
	assert.False(sharedPid)
	assert.False(testIsPidNamespacePresent(g))

	pidNs = pb.LinuxNamespace{
		Type: string(specs.PIDNamespace),
		Path: "/proc/112/ns/pid",
	}
	g.Linux.Namespaces = append(g.Linux.Namespaces, pidNs)

	sharedPid = k.handlePidNamespace(g, sandbox)
	assert.True(sharedPid)
	assert.False(testIsPidNamespacePresent(g))
}

func TestAgentConfigure(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "kata-agent-test")
	assert.Nil(err)
	defer os.RemoveAll(dir)

	k := &kataAgent{}
	h := &mockHypervisor{}
	c := KataAgentConfig{}
	id := "foobar"
	ctx := context.Background()

	err = k.configure(ctx, h, id, dir, c)
	assert.Nil(err)

	err = k.configure(ctx, h, id, dir, c)
	assert.Nil(err)
	assert.Empty(k.state.URL)

	err = k.configure(ctx, h, id, dir, c)
	assert.Nil(err)
}

func TestCmdToKataProcess(t *testing.T) {
	assert := assert.New(t)

	cmd := types.Cmd{
		Args:         strings.Split("foo", " "),
		Envs:         []types.EnvVar{},
		WorkDir:      "/",
		User:         "1000",
		PrimaryGroup: "1000",
	}
	_, err := cmdToKataProcess(cmd)
	assert.Nil(err)

	cmd1 := cmd
	cmd1.User = "foobar"
	_, err = cmdToKataProcess(cmd1)
	assert.Error(err)

	cmd1 = cmd
	cmd1.PrimaryGroup = "foobar"
	_, err = cmdToKataProcess(cmd1)
	assert.Error(err)

	cmd1 = cmd
	cmd1.User = "foobar:1000"
	_, err = cmdToKataProcess(cmd1)
	assert.Error(err)

	cmd1 = cmd
	cmd1.User = "1000:2000"
	_, err = cmdToKataProcess(cmd1)
	assert.Nil(err)

	cmd1 = cmd
	cmd1.SupplementaryGroups = []string{"foo"}
	_, err = cmdToKataProcess(cmd1)
	assert.Error(err)

	cmd1 = cmd
	cmd1.SupplementaryGroups = []string{"4000"}
	_, err = cmdToKataProcess(cmd1)
	assert.Nil(err)
}

func TestAgentCreateContainer(t *testing.T) {
	assert := assert.New(t)

	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "foobar",
		config: &SandboxConfig{
			ID:             "foobar",
			HypervisorType: MockHypervisor,
			HypervisorConfig: HypervisorConfig{
				KernelPath: "foo",
				ImagePath:  "bar",
			},
		},
		hypervisor: &mockHypervisor{},
	}

	store, err := persist.GetDriver()
	assert.NoError(err)
	assert.NotNil(store)
	sandbox.store = store

	container := &Container{
		ctx:       sandbox.ctx,
		id:        "barfoo",
		sandboxID: "foobar",
		sandbox:   sandbox,
		state: types.ContainerState{
			Fstype: "xfs",
		},
		config: &ContainerConfig{
			CustomSpec:  &specs.Spec{},
			Annotations: map[string]string{},
		},
	}

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	dir, err := ioutil.TempDir("", "kata-agent-test")
	assert.Nil(err)
	defer os.RemoveAll(dir)

	err = k.configure(context.Background(), &mockHypervisor{}, sandbox.id, dir, KataAgentConfig{})
	assert.Nil(err)

	// We'll fail on container metadata file creation, but it helps increasing coverage...
	_, err = k.createContainer(context.Background(), sandbox, container)
	assert.Error(err)
}

func TestAgentNetworkOperation(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	_, err = k.updateInterface(k.ctx, nil)
	assert.Nil(err)

	_, err = k.listInterfaces(k.ctx)
	assert.Nil(err)

	_, err = k.updateRoutes(k.ctx, []*pbTypes.Route{})
	assert.Nil(err)

	_, err = k.listRoutes(k.ctx)
	assert.Nil(err)
}

func TestKataGetAgentUrl(t *testing.T) {
	assert := assert.New(t)
	var err error

	k := &kataAgent{vmSocket: types.VSock{}}
	assert.NoError(err)
	url, err := k.getAgentURL()
	assert.Nil(err)
	assert.NotEmpty(url)

	k.vmSocket = types.HybridVSock{}
	assert.NoError(err)
	url, err = k.getAgentURL()
	assert.Nil(err)
	assert.NotEmpty(url)
}

func TestKataCopyFile(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	err = k.copyFile(context.Background(), "/abc/xyz/123", "/tmp")
	assert.Error(err)

	src, err := ioutil.TempFile("", "src")
	assert.NoError(err)
	defer os.Remove(src.Name())

	data := []byte("abcdefghi123456789")
	_, err = src.Write(data)
	assert.NoError(err)
	assert.NoError(src.Close())

	dst, err := ioutil.TempFile("", "dst")
	assert.NoError(err)
	assert.NoError(dst.Close())
	defer os.Remove(dst.Name())

	orgGrpcMaxDataSize := grpcMaxDataSize
	grpcMaxDataSize = 1
	defer func() {
		grpcMaxDataSize = orgGrpcMaxDataSize
	}()

	err = k.copyFile(context.Background(), src.Name(), dst.Name())
	assert.NoError(err)
}

func TestKataCleanupSandbox(t *testing.T) {
	assert := assert.New(t)

	kataHostSharedDirSaved := kataHostSharedDir
	kataHostSharedDir = func() string {
		td, _ := ioutil.TempDir("", "kata-cleanup")
		return td
	}
	defer func() {
		kataHostSharedDir = kataHostSharedDirSaved
	}()

	s := Sandbox{
		id: "testFoo",
	}

	dir := kataHostSharedDir()
	defer os.RemoveAll(dir)
	err := os.MkdirAll(path.Join(dir, s.id), 0777)
	assert.Nil(err)

	k := &kataAgent{ctx: context.Background()}
	k.cleanup(context.Background(), &s)

	_, err = os.Stat(dir)
	assert.False(os.IsExist(err))
}

func TestKataAgentKernelParams(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		debug             bool
		trace             bool
		containerPipeSize uint32
		traceMode         string
		traceType         string
		expectedParams    []Param
	}

	debugParam := Param{Key: "agent.log", Value: "debug"}

	traceIsolatedParam := Param{Key: "agent.trace", Value: "isolated"}
	traceCollatedParam := Param{Key: "agent.trace", Value: "collated"}

	traceFooParam := Param{Key: "agent.trace", Value: "foo"}

	containerPipeSizeParam := Param{Key: vcAnnotations.ContainerPipeSizeKernelParam, Value: "2097152"}

	data := []testData{
		{false, false, 0, "", "", []Param{}},
		{true, false, 0, "", "", []Param{debugParam}},

		{false, false, 0, "foo", "", []Param{}},
		{false, false, 0, "foo", "", []Param{}},
		{false, false, 0, "", "foo", []Param{}},
		{false, false, 0, "", "foo", []Param{}},
		{false, false, 0, "foo", "foo", []Param{}},
		{false, true, 0, "foo", "foo", []Param{}},

		{false, false, 0, agentTraceModeDynamic, "", []Param{}},
		{false, false, 0, agentTraceModeStatic, "", []Param{}},
		{false, false, 0, "", agentTraceTypeIsolated, []Param{}},
		{false, false, 0, "", agentTraceTypeCollated, []Param{}},
		{false, false, 0, "foo", agentTraceTypeIsolated, []Param{}},
		{false, false, 0, "foo", agentTraceTypeCollated, []Param{}},

		{false, false, 0, agentTraceModeDynamic, agentTraceTypeIsolated, []Param{}},
		{false, false, 0, agentTraceModeDynamic, agentTraceTypeCollated, []Param{}},

		{false, false, 0, agentTraceModeStatic, agentTraceTypeCollated, []Param{}},
		{false, false, 0, agentTraceModeStatic, agentTraceTypeCollated, []Param{}},

		{false, true, 0, agentTraceModeDynamic, agentTraceTypeIsolated, []Param{}},
		{false, true, 0, agentTraceModeDynamic, agentTraceTypeCollated, []Param{}},
		{true, true, 0, agentTraceModeDynamic, agentTraceTypeCollated, []Param{debugParam}},

		{false, true, 0, "", agentTraceTypeIsolated, []Param{}},
		{false, true, 0, "", agentTraceTypeCollated, []Param{}},
		{true, true, 0, "", agentTraceTypeIsolated, []Param{debugParam}},
		{true, true, 0, "", agentTraceTypeCollated, []Param{debugParam}},
		{false, true, 0, "foo", agentTraceTypeIsolated, []Param{}},
		{false, true, 0, "foo", agentTraceTypeCollated, []Param{}},
		{true, true, 0, "foo", agentTraceTypeIsolated, []Param{debugParam}},
		{true, true, 0, "foo", agentTraceTypeCollated, []Param{debugParam}},

		{false, true, 0, agentTraceModeStatic, agentTraceTypeIsolated, []Param{traceIsolatedParam}},
		{false, true, 0, agentTraceModeStatic, agentTraceTypeCollated, []Param{traceCollatedParam}},
		{true, true, 0, agentTraceModeStatic, agentTraceTypeIsolated, []Param{traceIsolatedParam, debugParam}},
		{true, true, 0, agentTraceModeStatic, agentTraceTypeCollated, []Param{traceCollatedParam, debugParam}},

		{false, true, 0, agentTraceModeStatic, "foo", []Param{traceFooParam}},
		{true, true, 0, agentTraceModeStatic, "foo", []Param{debugParam, traceFooParam}},

		{false, false, 0, "", "", []Param{}},
		{false, false, 2097152, "", "", []Param{containerPipeSizeParam}},
	}

	for i, d := range data {
		config := KataAgentConfig{
			Debug:             d.debug,
			Trace:             d.trace,
			TraceMode:         d.traceMode,
			TraceType:         d.traceType,
			ContainerPipeSize: d.containerPipeSize,
		}

		count := len(d.expectedParams)

		params := KataAgentKernelParams(config)

		if count == 0 {
			assert.Emptyf(params, "test %d (%+v)", i, d)
			continue
		}

		assert.Len(params, count)

		for _, p := range d.expectedParams {
			assert.Containsf(params, p, "test %d (%+v)", i, d)
		}
	}
}

func TestKataAgentHandleTraceSettings(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		traceMode               string
		trace                   bool
		expectDisableVMShutdown bool
		expectDynamicTracing    bool
	}

	data := []testData{
		{"", false, false, false},
		{"", true, false, false},
		{agentTraceModeStatic, true, true, false},
		{agentTraceModeDynamic, true, false, true},
	}

	for i, d := range data {
		k := &kataAgent{}

		config := KataAgentConfig{
			Trace:     d.trace,
			TraceMode: d.traceMode,
		}

		disableVMShutdown := k.handleTraceSettings(config)

		if d.expectDisableVMShutdown {
			assert.Truef(disableVMShutdown, "test %d (%+v)", i, d)
		} else {
			assert.Falsef(disableVMShutdown, "test %d (%+v)", i, d)
		}

		if d.expectDynamicTracing {
			assert.Truef(k.dynamicTracing, "test %d (%+v)", i, d)
		} else {
			assert.Falsef(k.dynamicTracing, "test %d (%+v)", i, d)
		}
	}
}

func TestKataAgentSetDefaultTraceConfigOptions(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		traceMode              string
		traceType              string
		trace                  bool
		expectDefaultTraceMode bool
		expectDefaultTraceType bool
		expectError            bool
	}

	data := []testData{
		{"", "", false, false, false, false},
		{agentTraceModeDynamic, agentTraceTypeCollated, false, false, false, false},
		{agentTraceModeDynamic, agentTraceTypeIsolated, false, false, false, false},
		{agentTraceModeStatic, agentTraceTypeCollated, false, false, false, false},
		{agentTraceModeStatic, agentTraceTypeIsolated, false, false, false, false},

		{agentTraceModeDynamic, agentTraceTypeCollated, true, false, false, false},
		{agentTraceModeDynamic, agentTraceTypeIsolated, true, false, false, false},

		{agentTraceModeStatic, agentTraceTypeCollated, true, false, false, false},
		{agentTraceModeStatic, agentTraceTypeIsolated, true, false, false, false},

		{agentTraceModeDynamic, "", true, false, true, false},
		{agentTraceModeDynamic, "invalid", true, false, false, true},

		{agentTraceModeStatic, "", true, false, true, false},
		{agentTraceModeStatic, "invalid", true, false, false, true},

		{"", agentTraceTypeIsolated, true, true, false, false},
		{"invalid", agentTraceTypeIsolated, true, false, false, true},

		{"", agentTraceTypeCollated, true, true, false, false},
		{"invalid", agentTraceTypeCollated, true, false, false, true},

		{"", "", true, true, true, false},
		{"invalid", "invalid", true, false, false, true},
	}

	for i, d := range data {
		config := &KataAgentConfig{
			Trace:     d.trace,
			TraceMode: d.traceMode,
			TraceType: d.traceType,
		}

		err := KataAgentSetDefaultTraceConfigOptions(config)
		if d.expectError {
			assert.Error(err, "test %d (%+v)", i, d)
			continue
		} else {
			assert.NoError(err, "test %d (%+v)", i, d)
		}

		if d.expectDefaultTraceMode {
			assert.Equalf(config.TraceMode, defaultAgentTraceMode, "test %d (%+v)", i, d)
		}

		if d.expectDefaultTraceType {
			assert.Equalf(config.TraceType, defaultAgentTraceType, "test %d (%+v)", i, d)
		}
	}
}

func TestKataAgentDirs(t *testing.T) {
	assert := assert.New(t)

	uidmapFile, err := os.OpenFile("/proc/self/uid_map", os.O_RDONLY, 0)
	assert.NoError(err)

	line, err := bufio.NewReader(uidmapFile).ReadBytes('\n')
	assert.NoError(err)

	uidmap := strings.Fields(string(line))
	expectedRootless := (uidmap[0] == "0" && uidmap[1] != "0")
	assert.Equal(expectedRootless, rootless.IsRootless())

	if expectedRootless {
		assert.Equal(kataHostSharedDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataHostSharedDir)
		assert.Equal(kataGuestSharedDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestSharedDir)
		assert.Equal(kataGuestSandboxDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestSandboxDir)
		assert.Equal(ephemeralPath(), os.Getenv("XDG_RUNTIME_DIR")+defaultEphemeralPath)
	} else {
		assert.Equal(kataHostSharedDir(), defaultKataHostSharedDir)
		assert.Equal(kataGuestSharedDir(), defaultKataGuestSharedDir)
		assert.Equal(kataGuestSandboxDir(), defaultKataGuestSandboxDir)
		assert.Equal(ephemeralPath(), defaultEphemeralPath)
	}
}

func TestSandboxBindMount(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}

	assert := assert.New(t)
	// create temporary files to mount:
	testMountPath, err := ioutil.TempDir("", "sandbox-test")
	assert.NoError(err)
	defer os.RemoveAll(testMountPath)

	// create a new shared directory for our test:
	kataHostSharedDirSaved := kataHostSharedDir
	testHostDir, err := ioutil.TempDir("", "kata-cleanup")
	assert.NoError(err)
	kataHostSharedDir = func() string {
		return testHostDir
	}
	defer func() {
		kataHostSharedDir = kataHostSharedDirSaved
	}()

	m1Path := filepath.Join(testMountPath, "foo.txt")
	f1, err := os.Create(m1Path)
	assert.NoError(err)
	defer f1.Close()

	m2Path := filepath.Join(testMountPath, "bar.txt")
	f2, err := os.Create(m2Path)
	assert.NoError(err)
	defer f2.Close()

	// create sandbox for mounting into
	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "foobar",
		config: &SandboxConfig{
			SandboxBindMounts: []string{m1Path, m2Path},
		},
	}
	k := &kataAgent{ctx: context.Background()}

	// make the shared directory for our test:
	dir := kataHostSharedDir()
	err = os.MkdirAll(path.Join(dir, sandbox.id), 0777)
	assert.Nil(err)
	defer os.RemoveAll(dir)

	sharePath := getSharePath(sandbox.id)
	mountPath := getMountPath(sandbox.id)

	err = os.MkdirAll(sharePath, DirMode)
	assert.Nil(err)
	err = os.MkdirAll(mountPath, DirMode)
	assert.Nil(err)

	// setup the expeted slave mount:
	err = bindMount(sandbox.ctx, mountPath, sharePath, true, "slave")
	assert.Nil(err)
	defer syscall.Unmount(sharePath, syscall.MNT_DETACH|UmountNoFollow)

	// Test the function. We expect it to succeed and for the mount to exist
	err = k.setupSandboxBindMounts(sandbox)
	assert.NoError(err)

	// Test the cleanup function. We expect it to succeed for the mount to be removed.
	err = k.cleanupSandboxBindMounts(sandbox)
	assert.NoError(err)

	// After successful cleanup, verify there are not any mounts left behind.
	stat := syscall.Stat_t{}
	mount1CheckPath := filepath.Join(getMountPath(sandbox.id), sandboxMountsDir, filepath.Base(m1Path))
	err = syscall.Stat(mount1CheckPath, &stat)
	assert.Error(err)
	assert.True(os.IsNotExist(err))

	mount2CheckPath := filepath.Join(getMountPath(sandbox.id), sandboxMountsDir, filepath.Base(m2Path))
	err = syscall.Stat(mount2CheckPath, &stat)
	assert.Error(err)
	assert.True(os.IsNotExist(err))

	// Now, let's setup the cleanup to fail. Setup the sandbox bind mount twice, which will result in
	// extra mounts being present that the sandbox description doesn't account for (ie, duplicate mounts).
	// We expect cleanup to fail on the first time, since it cannot remove the sandbox-bindmount directory because
	// there are leftover mounts.   If we run it a second time, however, it should succeed since it'll remove the
	// second set of mounts:
	err = k.setupSandboxBindMounts(sandbox)
	assert.NoError(err)
	err = k.setupSandboxBindMounts(sandbox)
	assert.NoError(err)
	// Test the cleanup function. We expect it to succeed for the mount to be removed.
	err = k.cleanupSandboxBindMounts(sandbox)
	assert.Error(err)
	err = k.cleanupSandboxBindMounts(sandbox)
	assert.NoError(err)

	//
	// Now, let's setup the sandbox bindmount to fail, and verify that no mounts are left behind
	//
	sandbox.config.SandboxBindMounts = append(sandbox.config.SandboxBindMounts, "oh-nos")
	err = k.setupSandboxBindMounts(sandbox)
	assert.Error(err)
	// Verify there aren't any mounts left behind
	stat = syscall.Stat_t{}
	err = syscall.Stat(mount1CheckPath, &stat)
	assert.Error(err)
	assert.True(os.IsNotExist(err))
	err = syscall.Stat(mount2CheckPath, &stat)
	assert.Error(err)
	assert.True(os.IsNotExist(err))

}
