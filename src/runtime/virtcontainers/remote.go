// (C) Copyright IBM Corp. 2022.
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
	annotations[hypannotations.DefaultVCPUs] = strconv.FormatUint(uint64(hypervisorConfig.NumVCPUs), 10)
	annotations[hypannotations.DefaultMemory] = strconv.FormatUint(uint64(hypervisorConfig.MemorySize), 10)
	annotations[hypannotations.VolumeName] = hypervisorConfig.VolumeName
	annotations[hypannotations.SRIOV] = strconv.FormatUint(uint64(hypervisorConfig.SRIOV), 10)
	annotations[hypannotations.VMType] = hypervisorConfig.VMType

	req := &pb.CreateVMRequest{
		Id:                   id,
		Annotations:          annotations,
		NetworkNamespacePath: network.NetworkID(),
	}

	res, err := s.client.CreateVM(context.Background(), req)
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

	// waitOnly doesn't make sense for remote hypervisor and suited for local hypervisor.
	// Instead use a similar logic like StartVM to handle StopVM with timeout.

	rh.sandboxID = remoteHypervisorSandboxID(rh.config.SandboxID)
	logrus.Printf("StopVM: sandboxID=%s", rh.sandboxID)

	timeout := defaultMinTimeout

	if rh.config.RemoteHypervisorTimeout > 0 {
		timeout = int(rh.config.RemoteHypervisorTimeout)
	}

	s, err := openRemoteService(rh.config.RemoteHypervisorSocket)
	if err != nil {
		return err
	}
	defer s.Close()

	req := &pb.StopVMRequest{
		Id: string(rh.sandboxID),
	}
	ctx2, cancel := context.WithTimeout(context.Background(), time.Duration(timeout)*time.Second)
	defer cancel()

	logrus.Printf("calling remote hypervisor StopVM (timeout: %d)", timeout)
	if _, err := s.client.StopVM(ctx2, req); err != nil {
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
	panic(notImplemented("PauseVM"))
}

func (rh *remoteHypervisor) SaveVM() error {
	panic(notImplemented("SaveVM"))
}

func (rh *remoteHypervisor) ResumeVM(ctx context.Context) error {
	panic(notImplemented("ResumeVM"))
}

func (rh *remoteHypervisor) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	// TODO
	logrus.Printf("addDevice: deviceType=%v devInfo=%#v", devType, devInfo)
	return nil
}

func (rh *remoteHypervisor) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	logrus.Printf("HotplugAddDevice: devInfo=%#v", devInfo)
	return "HotplugAddDevice is not implemented", nil
}

func (rh *remoteHypervisor) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	logrus.Printf("HotplugRemoveDevice: devInfo=%#v", devInfo)
	return "HotplugRemoveDevice is not implemented", nil
}

func (rh *remoteHypervisor) ResizeMemory(ctx context.Context, memMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {
	// TODO
	return memMB, MemoryDevice{}, nil
}

func (rh *remoteHypervisor) GetTotalMemoryMB(ctx context.Context) uint32 {
	return rh.config.MemorySize
}

func (rh *remoteHypervisor) ResizeVCPUs(ctx context.Context, vcpus uint32) (uint32, uint32, error) {
	// TODO
	return vcpus, vcpus, nil
}

func (rh *remoteHypervisor) GetVMConsole(ctx context.Context, sandboxID string) (string, string, error) {
	panic(notImplemented("GetVMConsole"))
}

func (rh *remoteHypervisor) Disconnect(ctx context.Context) {
	// TODO
	panic(notImplemented("Disconnect"))
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
	// TODO
	return VcpuThreadIDs{vcpus: make(map[int]int)}, nil
}

func (rh *remoteHypervisor) Cleanup(ctx context.Context) error {
	// TODO
	return nil
}

func (rh *remoteHypervisor) setConfig(config *HypervisorConfig) error {
	// Create a Validator specific for remote hypervisor
	rh.config = *config

	return nil
}

func (rh *remoteHypervisor) GetPids() []int {
	// TODO: meanwhile let's use shim pid as it used by crio to fetch start time
	return []int{os.Getpid()}
}

func (rh *remoteHypervisor) GetVirtioFsPid() *int {
	panic(notImplemented("GetVirtioFsPid"))
}

func (rh *remoteHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	panic(notImplemented("fromGrpc"))
}

func (rh *remoteHypervisor) toGrpc(ctx context.Context) ([]byte, error) {
	panic(notImplemented("toGrpc"))
}

func (rh *remoteHypervisor) Check() error {
	//TODO
	return nil
}

func (rh *remoteHypervisor) Save() persistapi.HypervisorState {
	// TODO
	// called from Sandbox.dumpHypervisor
	return persistapi.HypervisorState{}
}

func (rh *remoteHypervisor) Load(persistapi.HypervisorState) {
	// TODO
	// called from Sandbox.loadHypervisor
}

func (rh *remoteHypervisor) IsRateLimiterBuiltin() bool {
	// TODO
	return true
}
