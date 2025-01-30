// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"reflect"
	"strings"
	"syscall"
	"testing"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"

	volume "github.com/kata-containers/kata-containers/src/runtime/pkg/direct-volume"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	pb "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

const sysHugepagesDir = "/sys/kernel/mm/hugepages"

var (
	testBlkDriveFormat     = "testBlkDriveFormat"
	testBlockDeviceCtrPath = "testBlockDeviceCtrPath"
	testDevNo              = "testDevNo"
	testNvdimmID           = "testNvdimmID"
	testPCIPath, _         = types.PciPathFromString("04/02")
	testSCSIAddr           = "testSCSIAddr"
	testVirtPath           = "testVirtPath"
)

func TestKataAgentConnect(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

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
	defer mock.RemoveKataMockHybridVSock(url)

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
	defer mock.RemoveKataMockHybridVSock(url)

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
	mountSource := t.TempDir()

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
	mountSource := t.TempDir()

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
	var gid = 2000
	k := kataAgent{}

	// nolint: govet
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
		{
			BlockDeviceDriver: config.VirtioBlock,
			inputMount: Mount{
				FSGroup:             &gid,
				FSGroupChangePolicy: volume.FSGroupChangeOnRootMismatch,
			},
			inputDev: &drivers.BlockDevice{
				BlockDrive: &config.BlockDrive{
					PCIPath:  testPCIPath,
					VirtPath: testVirtPath,
				},
			},
			resultVol: &pb.Storage{
				Driver: kataBlkDevType,
				Source: testPCIPath.String(),
				FsGroup: &pb.FSGroup{
					GroupId:           uint32(gid),
					GroupChangePolicy: pbTypes.FSGroupChangePolicy_OnRootMismatch,
				},
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
	vPCIPath, err := types.PciPathFromString("01/02")
	assert.NoError(t, err)
	bPCIPath, err := types.PciPathFromString("03/04")
	assert.NoError(t, err)
	dPCIPath, err := types.PciPathFromString("04/05")
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
	dm := manager.NewDeviceManager(config.VirtioBlock, true, tmpDir, 0, devices)

	sConfig := SandboxConfig{}
	sConfig.HypervisorConfig.BlockDeviceDriver = config.VirtioBlock
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

	vStorage, err := k.createBlkStorageObject(c, vMount)
	assert.Nil(t, err, "Error while handling block volumes")
	bStorage, err := k.createBlkStorageObject(c, bMount)
	assert.Nil(t, err, "Error while handling block volumes")
	dStorage, err := k.createBlkStorageObject(c, dMount)
	assert.Nil(t, err, "Error while handling block volumes")

	vStorageExpected := &pb.Storage{
		MountPoint: vDestination,
		Fstype:     "bind",
		Options:    []string{"bind"},
		Driver:     kataBlkDevType,
		Source:     vPCIPath.String(),
	}
	bStorageExpected := &pb.Storage{
		MountPoint: bDestination,
		Fstype:     "bind",
		Options:    []string{"bind"},
		Driver:     kataBlkDevType,
		Source:     bPCIPath.String(),
	}
	dStorageExpected := &pb.Storage{
		MountPoint: dDestination,
		Fstype:     "ext4",
		Options:    []string{"ro"},
		Driver:     kataBlkDevType,
		Source:     dPCIPath.String(),
	}

	assert.Equal(t, vStorage, vStorageExpected, "Error while handle VhostUserBlk type block volume")
	assert.Equal(t, bStorage, bStorageExpected, "Error while handle BlockDevice type block volume")
	assert.Equal(t, dStorage, dStorageExpected, "Error while handle direct BlockDevice type block volume")
}

func TestAppendDevicesEmptyContainerDeviceList(t *testing.T) {
	k := kataAgent{}

	devList := []*pb.Device{}
	expected := []*pb.Device{}
	ctrDevices := []ContainerDevice{}

	c := &Container{
		sandbox: &Sandbox{
			devManager: manager.NewDeviceManager("virtio-scsi", false, "", 0, nil),
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

	testBlockDeviceID := "test-block-device"
	testCharacterDeviceId := "test-character-device"

	ctrDevices := []api.Device{
		&drivers.BlockDevice{
			GenericDevice: &drivers.GenericDevice{
				ID: testBlockDeviceID,
			},
			BlockDrive: &config.BlockDrive{
				PCIPath: testPCIPath,
			},
		},
		&drivers.GenericDevice{
			ID: testCharacterDeviceId,
		},
	}

	sandboxConfig := &SandboxConfig{
		HypervisorConfig: HypervisorConfig{
			BlockDeviceDriver: config.VirtioBlock,
		},
	}

	c := &Container{
		sandbox: &Sandbox{
			devManager: manager.NewDeviceManager("virtio-blk", false, "", 0, ctrDevices),
			config:     sandboxConfig,
		},
	}
	c.devices = append(
		c.devices,
		ContainerDevice{
			ID:            testBlockDeviceID,
			ContainerPath: testBlockDeviceCtrPath,
		},
		ContainerDevice{
			ID: testCharacterDeviceId,
		},
	)

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
			devManager: manager.NewDeviceManager("virtio-blk", true, testVhostUserStorePath, 0, ctrDevices),
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

func TestConstrainGRPCSpec(t *testing.T) {
	assert := assert.New(t)
	expectedCgroupPath := "system.slice:foo:bar"

	g := &pb.Spec{
		Hooks: &pb.Hooks{},
		Mounts: []*pb.Mount{
			{Destination: "/dev/shm"},
		},
		Linux: &pb.Linux{
			Seccomp: &pb.LinuxSeccomp{},
			Namespaces: []*pb.LinuxNamespace{
				{
					Type: string(specs.NetworkNamespace),
					Path: "/abc/123",
				},
				{
					Type: string(specs.MountNamespace),
					Path: "/abc/123",
				},
			},
			Resources: &pb.LinuxResources{
				Devices:        []*pb.LinuxDeviceCgroup{},
				Memory:         &pb.LinuxMemory{},
				CPU:            &pb.LinuxCPU{},
				Pids:           &pb.LinuxPids{},
				BlockIO:        &pb.LinuxBlockIO{},
				HugepageLimits: []*pb.LinuxHugepageLimit{},
				Network:        &pb.LinuxNetwork{},
			},
			CgroupsPath: "system.slice:foo:bar",
			Devices: []*pb.LinuxDevice{
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
	k.constrainGRPCSpec(g, true, true, "", true)

	// Check nil fields
	assert.Nil(g.Hooks)
	assert.NotNil(g.Linux.Seccomp)
	assert.Nil(g.Linux.Resources.Devices)
	assert.NotNil(g.Linux.Resources.Memory)
	assert.Nil(g.Linux.Resources.Pids)
	assert.Nil(g.Linux.Resources.BlockIO)
	assert.Len(g.Linux.Resources.HugepageLimits, 0)
	assert.Nil(g.Linux.Resources.Network)
	assert.NotNil(g.Linux.Resources.CPU)
	assert.Equal(g.Process.SelinuxLabel, "")

	// Check namespaces
	assert.Len(g.Linux.Namespaces, 1)
	assert.Empty(g.Linux.Namespaces[0].Path)

	// Check mounts
	assert.Len(g.Mounts, 1)

	// Check cgroup path
	assert.Equal(expectedCgroupPath, g.Linux.CgroupsPath)

	// Check Linux devices
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
	mountSource := t.TempDir()
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
			Namespaces: []*pb.LinuxNamespace{
				{
					Type: string(specs.NetworkNamespace),
					Path: "/abc/123",
				},
				{
					Type: string(specs.MountNamespace),
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
		Type: string(specs.UTSNamespace),
		Path: "",
	}

	g.Linux.Namespaces = append(g.Linux.Namespaces, &pidNs)
	g.Linux.Namespaces = append(g.Linux.Namespaces, &utsNs)

	sharedPid = k.handlePidNamespace(g, sandbox)
	assert.False(sharedPid)
	assert.False(testIsPidNamespacePresent(g))

	pidNs = pb.LinuxNamespace{
		Type: string(specs.PIDNamespace),
		Path: "/proc/112/ns/pid",
	}
	g.Linux.Namespaces = append(g.Linux.Namespaces, &pidNs)

	sharedPid = k.handlePidNamespace(g, sandbox)
	assert.True(sharedPid)
	assert.False(testIsPidNamespacePresent(g))
}

func TestAgentConfigure(t *testing.T) {
	assert := assert.New(t)

	dir := t.TempDir()

	k := &kataAgent{}
	h := &mockHypervisor{}
	c := KataAgentConfig{}
	id := "foobar"
	ctx := context.Background()

	err := k.configure(ctx, h, id, dir, c)
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
		agent:      newMockAgent(),
	}

	fsShare, err := NewFilesystemShare(sandbox)
	assert.Nil(err)
	sandbox.fsShare = fsShare

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
	defer mock.RemoveKataMockHybridVSock(url)

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

	dir := t.TempDir()

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
	defer mock.RemoveKataMockHybridVSock(url)

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
	defer mock.RemoveKataMockHybridVSock(url)

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

	src, err := os.CreateTemp("", "src")
	assert.NoError(err)
	defer os.Remove(src.Name())

	data := []byte("abcdefghi123456789")
	_, err = src.Write(data)
	assert.NoError(err)
	assert.NoError(src.Close())

	dst, err := os.CreateTemp("", "dst")
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

func TestKataCopyFileWithSymlink(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

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

	tempDir := t.TempDir()

	target := filepath.Join(tempDir, "target")
	err = os.WriteFile(target, []byte("abcdefghi123456789"), 0666)
	assert.NoError(err)

	symlink := filepath.Join(tempDir, "symlink")
	os.Symlink(target, symlink)

	dst, err := os.CreateTemp("", "dst")
	assert.NoError(err)
	assert.NoError(dst.Close())
	defer os.Remove(dst.Name())

	orgGrpcMaxDataSize := grpcMaxDataSize
	grpcMaxDataSize = 1
	defer func() {
		grpcMaxDataSize = orgGrpcMaxDataSize
	}()

	err = k.copyFile(context.Background(), symlink, dst.Name())
	assert.NoError(err)
}

func TestKataCleanupSandbox(t *testing.T) {
	assert := assert.New(t)

	kataHostSharedDirSaved := kataHostSharedDir
	kataHostSharedDir = func() string {
		return t.TempDir()
	}
	defer func() {
		kataHostSharedDir = kataHostSharedDirSaved
	}()

	s := Sandbox{
		id: "testFoo",
	}

	dir := kataHostSharedDir()
	err := os.MkdirAll(path.Join(dir, s.id), 0777)
	assert.Nil(err)

	k := &kataAgent{ctx: context.Background()}
	k.cleanup(context.Background())

	_, err = os.Stat(dir)
	assert.False(os.IsExist(err))
}

func TestKataAgentKernelParams(t *testing.T) {
	assert := assert.New(t)

	// nolint: govet
	type testData struct {
		debug             bool
		trace             bool
		containerPipeSize uint32
		expectedParams    []Param
	}

	debugParam := Param{Key: "agent.log", Value: "debug"}
	traceParam := Param{Key: "agent.trace", Value: "true"}

	containerPipeSizeParam := Param{Key: vcAnnotations.ContainerPipeSizeKernelParam, Value: "2097152"}

	data := []testData{
		{false, false, 0, []Param{}},

		// Debug
		{true, false, 0, []Param{debugParam}},

		// Tracing
		{false, true, 0, []Param{traceParam}},

		// Debug + Tracing
		{true, true, 0, []Param{debugParam, traceParam}},

		// pipesize
		{false, false, 2097152, []Param{containerPipeSizeParam}},

		// Debug + pipesize
		{true, false, 2097152, []Param{debugParam, containerPipeSizeParam}},

		// Tracing + pipesize
		{false, true, 2097152, []Param{traceParam, containerPipeSizeParam}},

		// Debug + Tracing + pipesize
		{true, true, 2097152, []Param{debugParam, traceParam, containerPipeSizeParam}},
	}

	for i, d := range data {
		config := KataAgentConfig{
			Debug:             d.debug,
			Trace:             d.trace,
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
		trace                   bool
		expectDisableVMShutdown bool
	}

	data := []testData{
		{false, false},
		{true, true},
	}

	for i, d := range data {
		k := &kataAgent{}

		config := KataAgentConfig{
			Trace: d.trace,
		}

		disableVMShutdown := k.handleTraceSettings(config)

		if d.expectDisableVMShutdown {
			assert.Truef(disableVMShutdown, "test %d (%+v)", i, d)
		} else {
			assert.Falsef(disableVMShutdown, "test %d (%+v)", i, d)
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
		assert.Equal(kataGuestNydusRootDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestNydusRootDir)
		assert.Equal(kataGuestNydusImageDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestNydusRootDir+"images"+"/")
		assert.Equal(kataGuestSharedDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestNydusRootDir+"containers"+"/")
	} else {
		assert.Equal(kataHostSharedDir(), defaultKataHostSharedDir)
		assert.Equal(kataGuestSharedDir(), defaultKataGuestSharedDir)
		assert.Equal(kataGuestSandboxDir(), defaultKataGuestSandboxDir)
		assert.Equal(ephemeralPath(), defaultEphemeralPath)
		assert.Equal(kataGuestNydusRootDir(), defaultKataGuestNydusRootDir)
		assert.Equal(kataGuestNydusImageDir(), defaultKataGuestNydusRootDir+"rafs"+"/")
		assert.Equal(kataGuestSharedDir(), defaultKataGuestNydusRootDir+"containers"+"/")
	}

	cid := "123"
	expected := "/rafs/123/lowerdir"
	assert.Equal(rafsMountPath(cid), expected)
}

func TestIsNydusRootFSType(t *testing.T) {
	testCases := map[string]bool{
		"nydus":                               false,
		"nydus-overlayfs":                     false,
		"fuse.nydus-overlayfs":                true,
		"fuse./usr/local/bin/nydus-overlayfs": true,
		"fuse.nydus-overlayfs-e0ae398a2":      true,
	}

	for test, exp := range testCases {
		t.Run(test, func(t *testing.T) {
			assert.Equal(t, exp, IsNydusRootFSType(test))
		})
	}
}

func TestKataAgentCreateContainerVFIODevices(t *testing.T) {
	assert := assert.New(t)

	// Create temporary directory to mock IOMMU filesystem
	tmpDir := t.TempDir()
	iommuPath := filepath.Join(tmpDir, "sys", "kernel", "iommu_groups", "2", "devices")
	err := os.MkdirAll(iommuPath, 0755)
	assert.NoError(err)

	// Create a dummy device file to satisfy the IOMMU group check
	dummyDev := filepath.Join(iommuPath, "0000:00:02.0")
	err = os.WriteFile(dummyDev, []byte(""), 0644)
	assert.NoError(err)

	// Save original paths and restore after test
	origConfigSysIOMMUPath := config.SysIOMMUGroupPath
	config.SysIOMMUGroupPath = filepath.Join(tmpDir, "sys", "kernel", "iommu_groups")
	defer func() {
		config.SysIOMMUGroupPath = origConfigSysIOMMUPath
	}()

	tests := []struct {
		name          string
		hotPlugVFIO   config.PCIePort
		coldPlugVFIO  config.PCIePort
		vfioMode      config.VFIOModeType
		expectVFIODev bool
	}{
		{
			name:          "VFIO device with cold plug enabled",
			hotPlugVFIO:   config.NoPort,
			coldPlugVFIO:  config.BridgePort,
			vfioMode:      config.VFIOModeVFIO,
			expectVFIODev: true,
		},
		{
			name:          "VFIO device with hot plug enabled",
			hotPlugVFIO:   config.BridgePort,
			coldPlugVFIO:  config.NoPort,
			vfioMode:      config.VFIOModeVFIO,
			expectVFIODev: true,
		},
		{
			name:          "VFIO device with cold plug enabled but guest kernel mode",
			hotPlugVFIO:   config.NoPort,
			coldPlugVFIO:  config.BridgePort,
			vfioMode:      config.VFIOModeGuestKernel,
			expectVFIODev: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			vfioGroup := "2"
			vfioPath := filepath.Join("/dev/vfio", vfioGroup)

			vfioDevice := config.DeviceInfo{
				ContainerPath: vfioPath,
				HostPath:      vfioPath,
				DevType:       "c",
				Major:         10,
				Minor:         196,
				ColdPlug:      tt.coldPlugVFIO != config.NoPort,
				Port:          tt.coldPlugVFIO,
			}

			// Setup container config with the VFIO device
			contConfig := &ContainerConfig{
				DeviceInfos: []config.DeviceInfo{vfioDevice},
			}

			// Create mock URL for kata agent
			url, err := mock.GenerateKataMockHybridVSock()
			assert.NoError(err)
			defer mock.RemoveKataMockHybridVSock(url)

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

			mockReceiver := &api.MockDeviceReceiver{}

			sandbox := &Sandbox{
				config: &SandboxConfig{
					VfioMode: tt.vfioMode,
					HypervisorConfig: HypervisorConfig{
						HotPlugVFIO:  tt.hotPlugVFIO,
						ColdPlugVFIO: tt.coldPlugVFIO,
					},
				},
				devManager: manager.NewDeviceManager("virtio-scsi", false, "", 0, nil),
				agent:      k,
			}

			container := &Container{
				sandbox: sandbox,
				id:      "test-container",
				config:  contConfig,
			}

			// Call createDevices which should trigger the full flow
			err = container.createDevices(contConfig)
			assert.NoError(err)

			// Find the device in device manager using the original device info
			dev := sandbox.devManager.FindDevice(&vfioDevice)

			if tt.expectVFIODev {
				// For cases where VFIO device should be handled
				assert.NotNil(dev, "VFIO device should be found in device manager")
				if dev != nil {
					// Manually attach the device to increase attach count
					err = dev.Attach(context.Background(), mockReceiver)
					assert.NoError(err, "Device attachment should succeed")

					assert.True(sandbox.devManager.IsDeviceAttached(dev.DeviceID()),
						"Device should be marked as attached")
				}
			} else {
				// For cases where VFIO device should be skipped
				assert.Nil(dev, "VFIO device should not be found in device manager")
			}
		})
	}
}
