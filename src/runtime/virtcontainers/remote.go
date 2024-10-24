// Copyright (c) 2022 IBM Corporation
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"context"
	"fmt"
	"net"
	"os"
	"strconv"
	"time"

	cri "github.com/containerd/containerd/pkg/cri/annotations"
	"github.com/containerd/ttrpc"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/hypervisor"
	hypannotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
)

const defaultMinTimeout = 60

type remoteHypervisor struct {
	sandboxID       remoteHypervisorSandboxID
	agentSocketPath string
	config          HypervisorConfig
}

type remoteHypervisorSandboxID string

type remoteService struct {
	conn   net.Conn
	client pb.HypervisorService
}

func openRemoteService(socketPath string) (*remoteService, error) {

	conn, err := net.Dial("unix", socketPath)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to remote hypervisor socket: %w", err)
	}

	ttrpcClient := ttrpc.NewClient(conn)

	client := pb.NewHypervisorClient(ttrpcClient)

	s := &remoteService{
		conn:   conn,
		client: client,
	}

	return s, nil
}

func (s *remoteService) Close() error {
	return s.conn.Close()
}

func (rh *remoteHypervisor) CreateVM(ctx context.Context, id string, network Network, hypervisorConfig *HypervisorConfig) error {

	rh.sandboxID = remoteHypervisorSandboxID(id)

	if err := rh.setConfig(hypervisorConfig); err != nil {
		return err
	}

	s, err := openRemoteService(hypervisorConfig.RemoteHypervisorSocket)
	if err != nil {
		return err
	}
	defer s.Close()

	annotations := map[string]string{}
	annotations[cri.SandboxName] = hypervisorConfig.SandboxName
	annotations[cri.SandboxNamespace] = hypervisorConfig.SandboxNamespace
	annotations[hypannotations.MachineType] = hypervisorConfig.HypervisorMachineType
	annotations[hypannotations.ImagePath] = hypervisorConfig.ImagePath
	annotations[hypannotations.DefaultVCPUs] = strconv.FormatUint(uint64(hypervisorConfig.NumVCPUs()), 10)
	annotations[hypannotations.DefaultMemory] = strconv.FormatUint(uint64(hypervisorConfig.MemorySize), 10)
	annotations[hypannotations.Initdata] = hypervisorConfig.Initdata
	annotations[hypannotations.DefaultGPUs] = strconv.FormatUint(uint64(hypervisorConfig.DefaultGPUs), 10)
	annotations[hypannotations.DefaultGPUModel] = hypervisorConfig.DefaultGPUModel

	req := &pb.CreateVMRequest{
		Id:                   id,
		Annotations:          annotations,
		NetworkNamespacePath: network.NetworkID(),
	}

	res, err := s.client.CreateVM(ctx, req)
	if err != nil {
		return fmt.Errorf("remote hypervisor call failed: %w", err)
	}

	if res.AgentSocketPath == "" {
		return errors.New("remote hypervisor does not return tunnel socket path")
	}

	rh.agentSocketPath = res.AgentSocketPath

	return nil
}

func (rh *remoteHypervisor) StartVM(ctx context.Context, timeout int) error {

	minTimeout := defaultMinTimeout
	if rh.config.RemoteHypervisorTimeout > 0 {
		minTimeout = int(rh.config.RemoteHypervisorTimeout)
	}

	if timeout < minTimeout {
		timeout = minTimeout
	}

	s, err := openRemoteService(rh.config.RemoteHypervisorSocket)
	if err != nil {
		return err
	}
	defer s.Close()

	req := &pb.StartVMRequest{
		Id: string(rh.sandboxID),
	}

	ctx2, cancel := context.WithTimeout(context.Background(), time.Duration(timeout)*time.Second)
	defer cancel()

	logrus.Printf("calling remote hypervisor StartVM (timeout: %d)", timeout)

	if _, err := s.client.StartVM(ctx2, req); err != nil {
		return fmt.Errorf("remote hypervisor call failed: %w", err)
	}

	return nil
}

func (rh *remoteHypervisor) AttestVM(ctx context.Context) error {
	return nil
}

func (rh *remoteHypervisor) StopVM(ctx context.Context, waitOnly bool) error {

	s, err := openRemoteService(rh.config.RemoteHypervisorSocket)
	if err != nil {
		return err
	}
	defer s.Close()

	req := &pb.StopVMRequest{
		Id: string(rh.sandboxID),
	}

	if _, err := s.client.StopVM(ctx, req); err != nil {
		return fmt.Errorf("remote hypervisor call failed: %w", err)
	}

	return nil
}

func (rh *remoteHypervisor) GenerateSocket(id string) (interface{}, error) {

	socketPath := rh.agentSocketPath
	if len(socketPath) == 0 {
		return nil, errors.New("failed to generate remote sock: TunnelSocketPath is not set")
	}

	remoteSock := types.RemoteSock{
		SandboxID:        id,
		TunnelSocketPath: socketPath,
	}

	return remoteSock, nil
}

func notImplemented(name string) error {

	err := errors.Errorf("%s: not implemented", name)

	logrus.Errorf(err.Error())

	if tracer, ok := err.(interface{ StackTrace() errors.StackTrace }); ok {
		for _, f := range tracer.StackTrace() {
			logrus.Errorf("%+s:%d\n", f, f)
		}
	}

	return err
}

func (rh *remoteHypervisor) PauseVM(ctx context.Context) error {
	return notImplemented("PauseVM")
}

func (rh *remoteHypervisor) SaveVM() error {
	return notImplemented("SaveVM")
}

func (rh *remoteHypervisor) ResumeVM(ctx context.Context) error {
	return notImplemented("ResumeVM")
}

func (rh *remoteHypervisor) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	// TODO should we return notImplemented("AddDevice"), rather than nil and ignoring it?
	logrus.Printf("addDevice: deviceType=%v devInfo=%#v", devType, devInfo)
	return nil
}

func (rh *remoteHypervisor) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	return nil, notImplemented("HotplugAddDevice")
}

func (rh *remoteHypervisor) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	return nil, notImplemented("HotplugRemoveDevice")
}

func (rh *remoteHypervisor) ResizeMemory(ctx context.Context, memMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {
	return memMB, MemoryDevice{}, nil
}

func (rh *remoteHypervisor) GetTotalMemoryMB(ctx context.Context) uint32 {
	//The remote hypervisor uses the peer pod config to determine the memory of the VM, so we need to use static resource management
	logrus.Error("GetTotalMemoryMB - remote hypervisor cannot update resources")
	return 0
}

func (rh *remoteHypervisor) ResizeVCPUs(ctx context.Context, vcpus uint32) (uint32, uint32, error) {
	return vcpus, vcpus, nil
}

func (rh *remoteHypervisor) GetVMConsole(ctx context.Context, sandboxID string) (string, string, error) {
	return "", "", notImplemented("GetVMConsole")
}

func (rh *remoteHypervisor) Disconnect(ctx context.Context) {
	notImplemented("Disconnect")
}

func (rh *remoteHypervisor) Capabilities(ctx context.Context) types.Capabilities {
	var caps types.Capabilities
	caps.SetBlockDeviceHotplugSupport()
	return caps
}

func (rh *remoteHypervisor) HypervisorConfig() HypervisorConfig {
	return rh.config
}

func (rh *remoteHypervisor) GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error) {
	// Not supported. return success
	// Just allocating an empty map
	return VcpuThreadIDs{}, nil
}

func (rh *remoteHypervisor) Cleanup(ctx context.Context) error {
	return nil
}

func (rh *remoteHypervisor) setConfig(config *HypervisorConfig) error {
	// Create a Validator specific for remote hypervisor
	rh.config = *config

	return nil
}

func (rh *remoteHypervisor) GetPids() []int {
	// let's use shim pid as it used by crio to fetch start time
	return []int{os.Getpid()}
}

func (rh *remoteHypervisor) GetVirtioFsPid() *int {
	return nil
}

func (rh *remoteHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	panic(notImplemented("fromGrpc"))
}

func (rh *remoteHypervisor) toGrpc(ctx context.Context) ([]byte, error) {
	panic(notImplemented("toGrpc"))
}

func (rh *remoteHypervisor) Check() error {
	return nil
}

func (rh *remoteHypervisor) Save() persistapi.HypervisorState {
	return persistapi.HypervisorState{}
}

func (rh *remoteHypervisor) Load(persistapi.HypervisorState) {
	notImplemented("Load")
}

func (rh *remoteHypervisor) IsRateLimiterBuiltin() bool {
	return false
}
