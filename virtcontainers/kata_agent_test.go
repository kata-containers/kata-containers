// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"syscall"
	"testing"

	gpb "github.com/gogo/protobuf/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"golang.org/x/net/context"
	"google.golang.org/grpc"

	aTypes "github.com/kata-containers/agent/pkg/types"
	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/runtime/virtcontainers/device/manager"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/mock"
	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
)

var (
	testKataProxyURLTempl  = "unix://%s/kata-proxy-test.sock"
	testBlockDeviceCtrPath = "testBlockDeviceCtrPath"
	testPCIAddr            = "04/02"
)

func proxyHandlerDiscard(c net.Conn) {
	buf := make([]byte, 1024)
	c.Read(buf)
}

func testGenerateKataProxySockDir() (string, error) {
	dir, err := ioutil.TempDir("", "kata-proxy-test")
	if err != nil {
		return "", err
	}

	return dir, nil
}

func TestKataAgentConnect(t *testing.T) {
	proxy := mock.ProxyUnixMock{
		ClientHandler: proxyHandlerDiscard,
	}

	sockDir, err := testGenerateKataProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	if err := proxy.Start(testKataProxyURL); err != nil {
		t.Fatal(err)
	}
	defer proxy.Stop()

	k := &kataAgent{
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	if err := k.connect(); err != nil {
		t.Fatal(err)
	}

	if k.client == nil {
		t.Fatal("Kata agent client is not properly initialized")
	}
}

func TestKataAgentDisconnect(t *testing.T) {
	proxy := mock.ProxyUnixMock{
		ClientHandler: proxyHandlerDiscard,
	}

	sockDir, err := testGenerateKataProxySockDir()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	if err := proxy.Start(testKataProxyURL); err != nil {
		t.Fatal(err)
	}
	defer proxy.Stop()

	k := &kataAgent{
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	if err := k.connect(); err != nil {
		t.Fatal(err)
	}

	if err := k.disconnect(); err != nil {
		t.Fatal(err)
	}

	if k.client != nil {
		t.Fatal("Kata agent client pointer should be nil")
	}
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

func (p *gRPCProxy) AddInterface(ctx context.Context, req *pb.AddInterfaceRequest) (*aTypes.Interface, error) {
	return nil, nil
}

func (p *gRPCProxy) RemoveInterface(ctx context.Context, req *pb.RemoveInterfaceRequest) (*aTypes.Interface, error) {
	return nil, nil
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

func gRPCRegister(s *grpc.Server, srv interface{}) {
	switch g := srv.(type) {
	case *gRPCProxy:
		pb.RegisterAgentServiceServer(s, g)
		pb.RegisterHealthServer(s, g)
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
		Type:   kataEphemeralDevType,
		Source: mountSource,
	}

	ociMounts = append(ociMounts, mount)
	epheStorages := k.handleEphemeralStorage(ociMounts)

	epheMountPoint := epheStorages[0].GetMountPoint()
	expected := filepath.Join(ephemeralPath, filepath.Base(mountSource))
	assert.Equal(t, epheMountPoint, expected,
		"Ephemeral mount point didn't match: got %s, expecting %s", epheMountPoint, expected)
}

func TestAppendDevicesEmptyContainerDeviceList(t *testing.T) {
	k := kataAgent{}

	devList := []*pb.Device{}
	expected := []*pb.Device{}
	ctrDevices := []ContainerDevice{}

	c := &Container{
		sandbox: &Sandbox{
			devManager: manager.NewDeviceManager("virtio-scsi", nil),
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
			devManager: manager.NewDeviceManager("virtio-blk", ctrDevices),
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
		},
	}

	constraintGRPCSpec(g, true)

	// check nil fields
	assert.Nil(g.Hooks)
	assert.Nil(g.Linux.Seccomp)
	assert.Nil(g.Linux.Resources.Devices)
	assert.NotNil(g.Linux.Resources.Memory)
	assert.Nil(g.Linux.Resources.Pids)
	assert.Nil(g.Linux.Resources.BlockIO)
	assert.Nil(g.Linux.Resources.HugepageLimits)
	assert.Nil(g.Linux.Resources.Network)
	assert.NotNil(g.Linux.Resources.CPU)

	// check namespaces
	assert.Len(g.Linux.Namespaces, 1)
	assert.Empty(g.Linux.Namespaces[0].Path)

	// check mounts
	assert.Len(g.Mounts, 1)

	// check cgroup path
	assert.Equal(expectedCgroupPath, g.Linux.CgroupsPath)
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
	assert.NotEmpty(g.Mounts[0].Source, filepath.Join(kataGuestSharedDir, shmDir))
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
	sandbox.state.Pid = 0

	k := kataAgent{}

	sharedPid, err := k.handlePidNamespace(g, sandbox)
	assert.Nil(err)
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

	sharedPid, err = k.handlePidNamespace(g, sandbox)
	assert.Nil(err)
	assert.False(sharedPid)
	assert.False(testIsPidNamespacePresent(g))

	sandbox.state.Pid = 112
	pidNs = pb.LinuxNamespace{
		Type: string(specs.PIDNamespace),
		Path: "/proc/112/ns/pid",
	}
	g.Linux.Namespaces = append(g.Linux.Namespaces, pidNs)

	sharedPid, err = k.handlePidNamespace(g, sandbox)
	assert.Nil(err)
	assert.True(sharedPid)
	assert.False(testIsPidNamespacePresent(g))

	// Arbitrary path
	pidNs = pb.LinuxNamespace{
		Type: string(specs.PIDNamespace),
		Path: "/proc/234/ns/pid",
	}
	g.Linux.Namespaces = append(g.Linux.Namespaces, pidNs)

	_, err = k.handlePidNamespace(g, sandbox)
	assert.NotNil(err)
}

func TestAgentPathAPI(t *testing.T) {
	assert := assert.New(t)

	k1 := &kataAgent{}
	k2 := &kataAgent{}
	id := "foobar"

	// getVMPath
	path1 := k1.getVMPath(id)
	path2 := k2.getVMPath(id)
	assert.Equal(path1, path2)

	// getSharePath
	path1 = k1.getSharePath(id)
	path2 = k2.getSharePath(id)
	assert.Equal(path1, path2)

	// generateVMSocket
	c := KataAgentConfig{}
	err := k1.generateVMSocket(id, c)
	assert.Nil(err)
	err = k2.generateVMSocket(id, c)
	assert.Nil(err)
	assert.Equal(k1, k2)

	err = k1.generateVMSocket(id, c)
	assert.Nil(err)
	_, ok := k1.vmSocket.(Socket)
	assert.True(ok)

	c.UseVSock = true
	err = k2.generateVMSocket(id, c)
	assert.Nil(err)
	_, ok = k2.vmSocket.(kataVSOCK)
	assert.True(ok)
}

func TestAgentConfigure(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "kata-agent-test")
	assert.Nil(err)

	k := &kataAgent{}
	h := &mockHypervisor{}
	c := KataAgentConfig{}
	id := "foobar"

	invalidAgent := HyperConfig{}
	err = k.configure(h, id, dir, true, invalidAgent)
	assert.Error(err)

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

	cmd := Cmd{
		Args:         strings.Split("foo", " "),
		Envs:         []EnvVar{},
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
		id: "foobar",
		config: &SandboxConfig{
			ID:             "foobar",
			HypervisorType: MockHypervisor,
			HypervisorConfig: HypervisorConfig{
				KernelPath: "foo",
				ImagePath:  "bar",
			},
		},
		hypervisor: &mockHypervisor{},
		storage:    &filesystem{},
	}

	container := &Container{
		id:        "barfoo",
		sandboxID: "foobar",
		sandbox:   sandbox,
		state: State{
			Fstype: "xfs",
		},
		config: &ContainerConfig{
			Annotations: map[string]string{},
		},
	}

	ociSpec, err := json.Marshal(&specs.Spec{})
	assert.Nil(err)
	container.config.Annotations[vcAnnotations.ConfigJSONKey] = string(ociSpec[:])

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
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	dir, err := ioutil.TempDir("", "kata-agent-test")
	assert.Nil(err)

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
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(sockDir)

	testKataProxyURL := fmt.Sprintf(testKataProxyURLTempl, sockDir)
	if err := proxy.Start(testKataProxyURL); err != nil {
		t.Fatal(err)
	}
	defer proxy.Stop()

	k := &kataAgent{
		state: KataAgentState{
			URL: testKataProxyURL,
		},
	}

	_, err = k.updateInterface(nil)
	assert.Nil(err)

	_, err = k.listInterfaces()
	assert.Nil(err)

	_, err = k.updateRoutes([]*types.Route{})
	assert.Nil(err)

	_, err = k.listRoutes()
	assert.Nil(err)
}

func TestKataAgentSetProxy(t *testing.T) {
	assert := assert.New(t)

	k := &kataAgent{}
	p := &kataBuiltInProxy{}
	s := &Sandbox{storage: &filesystem{}}

	err := k.setProxy(s, p, 0, "")
	assert.Error(err)

	err = k.setProxy(s, p, 0, "foobar")
	assert.Error(err)
}

func TestKataGetAgentUrl(t *testing.T) {
	assert := assert.New(t)

	k := &kataAgent{}
	err := k.generateVMSocket("foobar", KataAgentConfig{})
	assert.Nil(err)
	url, err := k.getAgentURL()
	assert.Nil(err)
	assert.NotEmpty(url)

	err = k.generateVMSocket("foobar", KataAgentConfig{UseVSock: true})
	assert.Nil(err)
	url, err = k.getAgentURL()
	assert.Nil(err)
	assert.NotEmpty(url)

}
