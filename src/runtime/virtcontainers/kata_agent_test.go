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

	"github.com/containerd/ttrpc"
	gpb "github.com/gogo/protobuf/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/manager"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	aTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	pb "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var (
	testKataProxyURLTempl  = "unix://%s/kata-proxy-test.sock"
	testBlockDeviceCtrPath = "testBlockDeviceCtrPath"
	testPCIAddr            = "04/02"
)

func testGenerateKataProxySockDir() (string, error) {
	dir, err := ioutil.TempDir("", "kata-proxy-test")
	if err != nil {
		return "", err
	}

	return dir, nil
}

func TestKataAgentConnect(t *testing.T) {
	assert := assert.New(t)
	proxy := mock.ProxyGRPCMock{
		GRPCImplementer: &gRPCProxy{},
		GRPCRegister:    gRPCRegister,
	}

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	err = proxy.Start(testKataProxyURL)
	assert.NoError(err)
	defer proxy.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	err = k.connect()
	assert.NoError(err)
	assert.NotNil(k.client)
}

func TestKataAgentDisconnect(t *testing.T) {
	assert := assert.New(t)
	proxy := mock.ProxyGRPCMock{
		GRPCImplementer: &gRPCProxy{},
		GRPCRegister:    gRPCRegister,
	}

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	err = proxy.Start(testKataProxyURL)
	assert.NoError(err)
	defer proxy.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	assert.NoError(k.connect())
	assert.NoError(k.disconnect())
	assert.Nil(k.client)
}

type gRPCProxy struct{}

var emptyResp = &gpb.Empty{}

func (p *gRPCProxy) CreateContainer(ctx context.Context, req *pb.CreateContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) StartContainer(ctx context.Context, req *pb.StartContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) ExecProcess(ctx context.Context, req *pb.ExecProcessRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) SignalProcess(ctx context.Context, req *pb.SignalProcessRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) WaitProcess(ctx context.Context, req *pb.WaitProcessRequest) (*pb.WaitProcessResponse, error) {
	return &pb.WaitProcessResponse{}, nil
}

func (p *gRPCProxy) ListProcesses(ctx context.Context, req *pb.ListProcessesRequest) (*pb.ListProcessesResponse, error) {
	return &pb.ListProcessesResponse{}, nil
}

func (p *gRPCProxy) UpdateContainer(ctx context.Context, req *pb.UpdateContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) RemoveContainer(ctx context.Context, req *pb.RemoveContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) WriteStdin(ctx context.Context, req *pb.WriteStreamRequest) (*pb.WriteStreamResponse, error) {
	return &pb.WriteStreamResponse{}, nil
}

func (p *gRPCProxy) ReadStdout(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	return &pb.ReadStreamResponse{}, nil
}

func (p *gRPCProxy) ReadStderr(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	return &pb.ReadStreamResponse{}, nil
}

func (p *gRPCProxy) CloseStdin(ctx context.Context, req *pb.CloseStdinRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) TtyWinResize(ctx context.Context, req *pb.TtyWinResizeRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) CreateSandbox(ctx context.Context, req *pb.CreateSandboxRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) DestroySandbox(ctx context.Context, req *pb.DestroySandboxRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) UpdateInterface(ctx context.Context, req *pb.UpdateInterfaceRequest) (*aTypes.Interface, error) {
	return &aTypes.Interface{}, nil
}

func (p *gRPCProxy) UpdateRoutes(ctx context.Context, req *pb.UpdateRoutesRequest) (*pb.Routes, error) {
	return &pb.Routes{}, nil
}

func (p *gRPCProxy) ListInterfaces(ctx context.Context, req *pb.ListInterfacesRequest) (*pb.Interfaces, error) {
	return &pb.Interfaces{}, nil
}

func (p *gRPCProxy) ListRoutes(ctx context.Context, req *pb.ListRoutesRequest) (*pb.Routes, error) {
	return &pb.Routes{}, nil
}

func (p *gRPCProxy) AddARPNeighbors(ctx context.Context, req *pb.AddARPNeighborsRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) OnlineCPUMem(ctx context.Context, req *pb.OnlineCPUMemRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) StatsContainer(ctx context.Context, req *pb.StatsContainerRequest) (*pb.StatsContainerResponse, error) {
	return &pb.StatsContainerResponse{}, nil
}

func (p *gRPCProxy) Check(ctx context.Context, req *pb.CheckRequest) (*pb.HealthCheckResponse, error) {
	return &pb.HealthCheckResponse{}, nil
}

func (p *gRPCProxy) Version(ctx context.Context, req *pb.CheckRequest) (*pb.VersionCheckResponse, error) {
	return &pb.VersionCheckResponse{}, nil

}

func (p *gRPCProxy) PauseContainer(ctx context.Context, req *pb.PauseContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) ResumeContainer(ctx context.Context, req *pb.ResumeContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) ReseedRandomDev(ctx context.Context, req *pb.ReseedRandomDevRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) GetGuestDetails(ctx context.Context, req *pb.GuestDetailsRequest) (*pb.GuestDetailsResponse, error) {
	return &pb.GuestDetailsResponse{}, nil
}

func (p *gRPCProxy) SetGuestDateTime(ctx context.Context, req *pb.SetGuestDateTimeRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *gRPCProxy) CopyFile(ctx context.Context, req *pb.CopyFileRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *gRPCProxy) StartTracing(ctx context.Context, req *pb.StartTracingRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *gRPCProxy) StopTracing(ctx context.Context, req *pb.StopTracingRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *gRPCProxy) MemHotplugByProbe(ctx context.Context, req *pb.MemHotplugByProbeRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func gRPCRegister(s *ttrpc.Server, srv interface{}) {
	switch g := srv.(type) {
	case *gRPCProxy:
		pb.RegisterAgentServiceService(s, g)
		pb.RegisterHealthService(s, g)
	}
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

	impl := &gRPCProxy{}

	proxy := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}

	sockDir, err := testGenerateKataProxySockDir()
	assert.Nil(err)
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	err = proxy.Start(testKataProxyURL)
	assert.Nil(err)
	defer proxy.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	for _, req := range reqList {
		_, err = k.sendReq(req)
		assert.Nil(err)
	}

	sandbox := &Sandbox{}
	container := &Container{}
	execid := "processFooBar"

	err = k.startContainer(sandbox, container)
	assert.Nil(err)

	err = k.signalProcess(container, execid, syscall.SIGKILL, true)
	assert.Nil(err)

	err = k.winsizeProcess(container, execid, 100, 200)
	assert.Nil(err)

	_, err = k.processListContainer(sandbox, Container{}, ProcessListOptions{})
	assert.Nil(err)

	err = k.updateContainer(sandbox, Container{}, specs.LinuxResources{})
	assert.Nil(err)

	err = k.pauseContainer(sandbox, Container{})
	assert.Nil(err)

	err = k.resumeContainer(sandbox, Container{})
	assert.Nil(err)

	err = k.onlineCPUMem(1, true)
	assert.Nil(err)

	_, err = k.statsContainer(sandbox, Container{})
	assert.Nil(err)

	err = k.check()
	assert.Nil(err)

	_, err = k.waitProcess(container, execid)
	assert.Nil(err)

	_, err = k.writeProcessStdin(container, execid, []byte{'c'})
	assert.Nil(err)

	err = k.closeProcessStdin(container, execid)
	assert.Nil(err)

	_, err = k.readProcessStdout(container, execid, []byte{})
	assert.Nil(err)

	_, err = k.readProcessStderr(container, execid, []byte{})
	assert.Nil(err)
}

func TestHandleEphemeralStorage(t *testing.T) {
	k := kataAgent{}
	var ociMounts []specs.Mount
	mountSource := "/tmp/mountPoint"

	mount := specs.Mount{
		Type:   KataEphemeralDevType,
		Source: mountSource,
	}

	ociMounts = append(ociMounts, mount)
	epheStorages := k.handleEphemeralStorage(ociMounts)

	epheMountPoint := epheStorages[0].MountPoint
	expected := filepath.Join(ephemeralPath(), filepath.Base(mountSource))
	assert.Equal(t, epheMountPoint, expected,
		"Ephemeral mount point didn't match: got %s, expecting %s", epheMountPoint, expected)
}

func TestHandleLocalStorage(t *testing.T) {
	k := kataAgent{}
	var ociMounts []specs.Mount
	mountSource := "mountPoint"

	mount := specs.Mount{
		Type:   KataLocalDevType,
		Source: mountSource,
	}

	sandboxID := "sandboxid"
	rootfsSuffix := "rootfs"

	ociMounts = append(ociMounts, mount)
	localStorages := k.handleLocalStorage(ociMounts, sandboxID, rootfsSuffix)

	assert.NotNil(t, localStorages)
	assert.Equal(t, len(localStorages), 1)

	localMountPoint := localStorages[0].MountPoint
	expected := filepath.Join(kataGuestSharedDir(), sandboxID, rootfsSuffix, KataLocalDevType, filepath.Base(mountSource))
	assert.Equal(t, localMountPoint, expected)
}

func TestHandleBlockVolume(t *testing.T) {
	k := kataAgent{}

	c := &Container{
		id: "100",
	}
	containers := map[string]*Container{}
	containers[c.id] = c

	// Create a VhostUserBlk device and a DeviceBlock device
	vDevID := "MockVhostUserBlk"
	bDevID := "MockDeviceBlock"
	vDestination := "/VhostUserBlk/destination"
	bDestination := "/DeviceBlock/destination"
	vPCIAddr := "0001:01"
	bPCIAddr := "0002:01"

	vDev := drivers.NewVhostUserBlkDevice(&config.DeviceInfo{ID: vDevID})
	bDev := drivers.NewBlockDevice(&config.DeviceInfo{ID: bDevID})

	vDev.VhostUserDeviceAttrs = &config.VhostUserDeviceAttrs{PCIAddr: vPCIAddr}
	bDev.BlockDrive = &config.BlockDrive{PCIAddr: bPCIAddr}

	var devices []api.Device
	devices = append(devices, vDev, bDev)

	// Create a VhostUserBlk mount and a DeviceBlock mount
	var mounts []Mount
	vMount := Mount{
		BlockDeviceID: vDevID,
		Destination:   vDestination,
	}
	bMount := Mount{
		BlockDeviceID: bDevID,
		Destination:   bDestination,
	}
	mounts = append(mounts, vMount, bMount)

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
		Source:     vPCIAddr,
	}
	bStorage := &pb.Storage{
		MountPoint: bDestination,
		Fstype:     "bind",
		Options:    []string{"bind"},
		Driver:     kataBlkDevType,
		Source:     bPCIAddr,
	}

	assert.Equal(t, vStorage, volumeStorages[0], "Error while handle VhostUserBlk type block volume")
	assert.Equal(t, bStorage, volumeStorages[1], "Error while handle BlockDevice type block volume")
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
				PCIAddr: testPCIAddr,
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
			Id:            testPCIAddr,
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
				PCIAddr: testPCIAddr,
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
			Id:            testPCIAddr,
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

	g := &pb.Spec{
		Hooks: &pb.Hooks{},
		Mounts: []pb.Mount{
			{Destination: "/dev/shm"},
		},
	}

	k.handleShm(g, sandbox)

	assert.Len(g.Mounts, 1)
	assert.NotEmpty(g.Mounts[0].Destination)
	assert.Equal(g.Mounts[0].Destination, "/dev/shm")
	assert.Equal(g.Mounts[0].Type, "bind")
	assert.NotEmpty(g.Mounts[0].Source, filepath.Join(kataGuestSharedDir(), shmDir))
	assert.Equal(g.Mounts[0].Options, []string{"rbind"})

	sandbox.shmSize = 0
	k.handleShm(g, sandbox)

	assert.Len(g.Mounts, 1)
	assert.NotEmpty(g.Mounts[0].Destination)
	assert.Equal(g.Mounts[0].Destination, "/dev/shm")
	assert.Equal(g.Mounts[0].Type, "tmpfs")
	assert.Equal(g.Mounts[0].Source, "shm")

	sizeOption := fmt.Sprintf("size=%d", DefaultShmSize)
	assert.Equal(g.Mounts[0].Options, []string{"noexec", "nosuid", "nodev", "mode=1777", sizeOption})
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

func TestAgentPathAPI(t *testing.T) {
	assert := assert.New(t)

	k1 := &kataAgent{}
	k2 := &kataAgent{}
	id := "foobar"

	// getSharePath
	path1 := k1.getSharePath(id)
	path2 := k2.getSharePath(id)
	assert.Equal(path1, path2)
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

	err = k.configure(h, id, dir, true, c)
	assert.Nil(err)

	err = k.configure(h, id, dir, true, c)
	assert.Nil(err)
	assert.Empty(k.state.URL)

	err = k.configure(h, id, dir, false, c)
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

	newStore, err := persist.GetDriver()
	assert.NoError(err)
	assert.NotNil(newStore)
	sandbox.newStore = newStore

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

	impl := &gRPCProxy{}

	proxy := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}

	sockDir, err := testGenerateKataProxySockDir()
	assert.Nil(err)
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	err = proxy.Start(testKataProxyURL)
	assert.Nil(err)
	defer proxy.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	dir, err := ioutil.TempDir("", "kata-agent-test")
	assert.Nil(err)
	defer os.RemoveAll(dir)

	err = k.configure(&mockHypervisor{}, sandbox.id, dir, true, KataAgentConfig{})
	assert.Nil(err)

	// We'll fail on container metadata file creation, but it helps increasing coverage...
	_, err = k.createContainer(sandbox, container)
	assert.Error(err)
}

func TestAgentNetworkOperation(t *testing.T) {
	assert := assert.New(t)

	impl := &gRPCProxy{}

	proxy := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	assert.NoError(proxy.Start(testKataProxyURL))
	defer proxy.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	_, err = k.updateInterface(nil)
	assert.Nil(err)

	_, err = k.listInterfaces()
	assert.Nil(err)

	_, err = k.updateRoutes([]*vcTypes.Route{})
	assert.Nil(err)

	_, err = k.listRoutes()
	assert.Nil(err)
}

func TestKataAgentSetProxy(t *testing.T) {
	assert := assert.New(t)

	k := &kataAgent{ctx: context.Background()}
	p := &kataBuiltInProxy{}
	s := &Sandbox{
		ctx: context.Background(),
		id:  "foobar",
	}

	err := k.setProxy(s, p, 0, "")
	assert.Error(err)
}

func TestKataGetAgentUrl(t *testing.T) {
	assert := assert.New(t)
	var err error

	k := &kataAgent{vmSocket: types.Socket{HostPath: "/abc"}}
	assert.NoError(err)
	url, err := k.getAgentURL()
	assert.Nil(err)
	assert.NotEmpty(url)

	k.vmSocket = types.VSock{}
	assert.NoError(err)
	url, err = k.getAgentURL()
	assert.Nil(err)
	assert.NotEmpty(url)
}

func TestKataCopyFile(t *testing.T) {
	assert := assert.New(t)

	impl := &gRPCProxy{}

	proxy := mock.ProxyGRPCMock{
		GRPCImplementer: impl,
		GRPCRegister:    gRPCRegister,
	}

	sockDir, err := testGenerateKataProxySockDir()
	assert.NoError(err)
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	err = proxy.Start(testKataProxyURL)
	assert.NoError(err)
	defer proxy.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	err = k.copyFile("/abc/xyz/123", "/tmp")
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

	err = k.copyFile(src.Name(), dst.Name())
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
	k.cleanup(&s)

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
