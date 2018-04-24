// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"reflect"
	"testing"

	gpb "github.com/gogo/protobuf/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"github.com/vishvananda/netlink"
	"golang.org/x/net/context"
	"google.golang.org/grpc"

	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/mock"
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

func (p *gRPCProxy) AddInterface(ctx context.Context, req *pb.AddInterfaceRequest) (*pb.Interface, error) {
	return nil, nil
}

func (p *gRPCProxy) RemoveInterface(ctx context.Context, req *pb.RemoveInterfaceRequest) (*pb.Interface, error) {
	return nil, nil
}

func (p *gRPCProxy) UpdateInterface(ctx context.Context, req *pb.UpdateInterfaceRequest) (*pb.Interface, error) {
	return nil, nil
}

func (p *gRPCProxy) UpdateRoutes(ctx context.Context, req *pb.UpdateRoutesRequest) (*pb.Routes, error) {
	return nil, nil
}

func (p *gRPCProxy) OnlineCPUMem(ctx context.Context, req *pb.OnlineCPUMemRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *gRPCProxy) Check(ctx context.Context, req *pb.CheckRequest) (*pb.HealthCheckResponse, error) {
	return &pb.HealthCheckResponse{}, nil
}

func (p *gRPCProxy) Version(ctx context.Context, req *pb.CheckRequest) (*pb.VersionCheckResponse, error) {
	return &pb.VersionCheckResponse{}, nil

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
}

func TestKataAgentSendReq(t *testing.T) {
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

	for _, req := range reqList {
		if _, err := k.sendReq(req); err != nil {
			t.Fatal(err)
		}
	}
}

func TestGenerateInterfacesAndRoutes(t *testing.T) {

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

	//
	//Create a couple of addresses
	//
	address1 := &net.IPNet{IP: net.IPv4(172, 17, 0, 2), Mask: net.CIDRMask(16, 32)}
	address2 := &net.IPNet{IP: net.IPv4(182, 17, 0, 2), Mask: net.CIDRMask(16, 32)}

	addrs := []netlink.Addr{
		{IPNet: address1, Label: "phyaddr1"},
		{IPNet: address2, Label: "phyaddr2"},
	}

	// Create a couple of routes:
	dst2 := &net.IPNet{IP: net.IPv4(172, 17, 0, 0), Mask: net.CIDRMask(16, 32)}
	src2 := net.IPv4(172, 17, 0, 2)
	gw2 := net.IPv4(172, 17, 0, 1)

	routes := []netlink.Route{
		{LinkIndex: 329, Dst: nil, Src: nil, Gw: net.IPv4(172, 17, 0, 1), Scope: netlink.Scope(254)},
		{LinkIndex: 329, Dst: dst2, Src: src2, Gw: gw2},
	}

	networkInfo := NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{MTU: 1500},
			Type:      "",
		},
		Addrs:  addrs,
		Routes: routes,
	}

	ep0 := &PhysicalEndpoint{
		IfaceName:          "eth0",
		HardAddr:           net.HardwareAddr{0x02, 0x00, 0xca, 0xfe, 0x00, 0x04}.String(),
		EndpointProperties: networkInfo,
	}

	endpoints := []Endpoint{ep0}

	nns := NetworkNamespace{NetNsPath: "foobar", NetNsCreated: true, Endpoints: endpoints}

	resInterfaces, resRoutes, err := k.generateInterfacesAndRoutes(nns)

	//
	// Build expected results:
	//
	expectedAddresses := []*pb.IPAddress{
		{Family: 0, Address: "172.17.0.2", Mask: "16"},
		{Family: 0, Address: "182.17.0.2", Mask: "16"},
	}

	expectedInterfaces := []*pb.Interface{
		{Device: "eth0", Name: "eth0", IPAddresses: expectedAddresses, Mtu: 1500, HwAddr: "02:00:ca:fe:00:04"},
	}

	expectedRoutes := []*pb.Route{
		{Dest: "", Gateway: "172.17.0.1", Device: "eth0", Source: "", Scope: uint32(254)},
		{Dest: "172.17.0.0/16", Gateway: "172.17.0.1", Device: "eth0", Source: "172.17.0.2"},
	}

	assert.Nil(t, err, "unexpected failure when calling generateKataInterfacesAndRoutes")
	assert.True(t, reflect.DeepEqual(resInterfaces, expectedInterfaces),
		"Interfaces returned didn't match: got %+v, expecting %+v", resInterfaces, expectedInterfaces)
	assert.True(t, reflect.DeepEqual(resRoutes, expectedRoutes),
		"Routes returned didn't match: got %+v, expecting %+v", resRoutes, expectedRoutes)

}

func TestAppendDevicesEmptyContainerDeviceList(t *testing.T) {
	k := kataAgent{}

	devList := []*pb.Device{}
	expected := []*pb.Device{}
	ctrDevices := []api.Device{}

	updatedDevList := k.appendDevices(devList, ctrDevices)
	assert.True(t, reflect.DeepEqual(updatedDevList, expected),
		"Device lists didn't match: got %+v, expecting %+v",
		updatedDevList, expected)
}

func TestAppendDevices(t *testing.T) {
	k := kataAgent{}

	devList := []*pb.Device{}
	expected := []*pb.Device{
		{
			Type:          kataBlkDevType,
			ContainerPath: testBlockDeviceCtrPath,
			Id:            testPCIAddr,
		},
	}
	ctrDevices := []api.Device{
		&drivers.BlockDevice{
			DeviceInfo: config.DeviceInfo{
				ContainerPath: testBlockDeviceCtrPath,
			},
			PCIAddr: testPCIAddr,
		},
	}

	updatedDevList := k.appendDevices(devList, ctrDevices)
	assert.True(t, reflect.DeepEqual(updatedDevList, expected),
		"Device lists didn't match: got %+v, expecting %+v",
		updatedDevList, expected)
}

func TestConstraintGRPCSpec(t *testing.T) {
	assert := assert.New(t)

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
		},
	}

	constraintGRPCSpec(g)

	// check nil fields
	assert.Nil(g.Hooks)
	assert.Nil(g.Linux.Seccomp)
	assert.Nil(g.Linux.Resources.Devices)
	assert.Nil(g.Linux.Resources.Memory)
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
	assert.NotEmpty(g.Mounts[0].Destination)
	assert.NotEmpty(g.Mounts[0].Type)
	assert.NotEmpty(g.Mounts[0].Source)
	assert.NotEmpty(g.Mounts[0].Options)
}
