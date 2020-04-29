// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// gRPC mock server

package mockserver

import (
	"sync"

	"github.com/gogo/protobuf/types"
	"golang.org/x/net/context"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"

	pbTypes "github.com/kata-containers/agent/pkg/types"
	pb "github.com/kata-containers/agent/protocols/grpc"
)

const (
	// MockServerVersion specifies the version of the fake server
	MockServerVersion = "mock.0.1"
)

// If an rpc changes any pod/container/process, take a write lock.
var mockLock sync.RWMutex

type pod struct {
	containers map[string]*container
}

// container init process pid is always
type container struct {
	id      string
	initPid string
	proc    map[string]*process
}

type process struct {
	pid  string
	proc *pb.Process
}

type mockServer struct {
	pod *pod
}

// NewMockServer creates a new gRPC server
func NewMockServer() *grpc.Server {
	mock := &mockServer{}
	serv := grpc.NewServer()
	pb.RegisterAgentServiceServer(serv, mock)
	pb.RegisterHealthServer(serv, mock)

	return serv
}

func validateOCISpec(spec *pb.Spec) error {
	if spec == nil || spec.Process == nil {
		return status.Error(codes.InvalidArgument, "invalid container spec")
	}
	return nil
}

func (m *mockServer) checkExist(containerID, execID string, createContainer, checkProcess bool) error {
	if m.pod == nil {
		return status.Error(codes.NotFound, "pod not created")
	}
	if containerID == "" {
		return status.Error(codes.InvalidArgument, "container ID must be set")
	}
	if checkProcess && execID == "0" {
		return status.Error(codes.InvalidArgument, "process ID must be set")
	}

	// Check container existence
	if createContainer {
		if m.pod.containers[containerID] != nil {
			return status.Errorf(codes.AlreadyExists, "container ID %s already taken", containerID)
		}
		return nil
	} else if m.pod.containers[containerID] == nil {
		return status.Errorf(codes.NotFound, "container %s does not exist", containerID)
	}

	// Check process existence
	if checkProcess {
		c := m.pod.containers[containerID]
		if c.proc[execID] == nil {
			return status.Errorf(codes.NotFound, "process %s does not exist", execID)
		}
	}

	return nil
}

func (m *mockServer) processExist(containerID string, execID string) error {
	return m.checkExist(containerID, execID, false, true)
}

func (m *mockServer) containerExist(containerID string) error {
	return m.checkExist(containerID, "0", false, false)
}

func (m *mockServer) containerNonExist(containerID string) error {
	return m.checkExist(containerID, "0", true, false)
}

func (m *mockServer) podExist() error {
	if m.pod == nil {
		return status.Error(codes.NotFound, "pod not created")
	}
	return nil
}

func (m *mockServer) Check(ctx context.Context, req *pb.CheckRequest) (*pb.HealthCheckResponse, error) {
	return &pb.HealthCheckResponse{Status: pb.HealthCheckResponse_SERVING}, nil
}

func (m *mockServer) Version(ctx context.Context, req *pb.CheckRequest) (*pb.VersionCheckResponse, error) {
	return &pb.VersionCheckResponse{
		GrpcVersion:  pb.APIVersion,
		AgentVersion: MockServerVersion,
	}, nil
}

func (m *mockServer) CreateContainer(ctx context.Context, req *pb.CreateContainerRequest) (*types.Empty, error) {
	mockLock.Lock()
	defer mockLock.Unlock()
	if err := m.containerNonExist(req.ContainerId); err != nil {
		return nil, err
	}

	if err := validateOCISpec(req.OCI); err != nil {
		return nil, err
	}

	c := &container{
		id:   req.ContainerId,
		proc: make(map[string]*process),
	}
	c.initPid = req.ExecId
	c.proc[c.initPid] = &process{
		pid:  c.initPid,
		proc: req.OCI.Process,
	}
	m.pod.containers[req.ContainerId] = c

	return &types.Empty{}, nil
}

func (m *mockServer) StartContainer(ctx context.Context, req *pb.StartContainerRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.containerExist(req.ContainerId); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}

func (m *mockServer) RemoveContainer(ctx context.Context, req *pb.RemoveContainerRequest) (*types.Empty, error) {
	mockLock.Lock()
	defer mockLock.Unlock()
	if err := m.containerExist(req.ContainerId); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}

func (m *mockServer) ExecProcess(ctx context.Context, req *pb.ExecProcessRequest) (*types.Empty, error) {
	mockLock.Lock()
	defer mockLock.Unlock()
	if err := m.containerExist(req.ContainerId); err != nil {
		return nil, err
	}

	c := m.pod.containers[req.ContainerId]
	c.proc[req.ExecId] = &process{
		pid:  req.ExecId,
		proc: req.Process,
	}
	return &types.Empty{}, nil
}

func (m *mockServer) SignalProcess(ctx context.Context, req *pb.SignalProcessRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.processExist(req.ContainerId, req.ExecId); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}

func (m *mockServer) WaitProcess(ctx context.Context, req *pb.WaitProcessRequest) (*pb.WaitProcessResponse, error) {
	mockLock.Lock()
	defer mockLock.Unlock()
	if err := m.processExist(req.ContainerId, req.ExecId); err != nil {
		return nil, err
	}

	// remove process once it is waited
	c := m.pod.containers[req.ContainerId]
	c.proc[req.ExecId] = nil
	// container gone, clean it up
	if c.initPid == req.ExecId {
		m.pod.containers[req.ContainerId] = nil
	}

	return &pb.WaitProcessResponse{Status: 0}, nil
}

func (m *mockServer) WriteStdin(ctx context.Context, req *pb.WriteStreamRequest) (*pb.WriteStreamResponse, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.processExist(req.ContainerId, req.ExecId); err != nil {
		return nil, err
	}

	return &pb.WriteStreamResponse{Len: uint32(len(req.Data))}, nil
}

func (m *mockServer) ReadStdout(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.processExist(req.ContainerId, req.ExecId); err != nil {
		return nil, err
	}

	return &pb.ReadStreamResponse{}, nil
}

func (m *mockServer) ReadStderr(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.processExist(req.ContainerId, req.ExecId); err != nil {
		return nil, err
	}

	return &pb.ReadStreamResponse{}, nil
}

func (m *mockServer) CloseStdin(ctx context.Context, req *pb.CloseStdinRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.processExist(req.ContainerId, req.ExecId); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}

func (m *mockServer) TtyWinResize(ctx context.Context, req *pb.TtyWinResizeRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.processExist(req.ContainerId, req.ExecId); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}

func (m *mockServer) CreateSandbox(ctx context.Context, req *pb.CreateSandboxRequest) (*types.Empty, error) {
	mockLock.Lock()
	defer mockLock.Unlock()
	if m.pod != nil {
		return nil, status.Error(codes.AlreadyExists, "pod already created")
	}
	m.pod = &pod{
		containers: make(map[string]*container),
	}
	return &types.Empty{}, nil
}

func (m *mockServer) DestroySandbox(ctx context.Context, req *pb.DestroySandboxRequest) (*types.Empty, error) {
	mockLock.Lock()
	defer mockLock.Unlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	m.pod = nil
	return &types.Empty{}, nil
}

func (m *mockServer) UpdateInterface(ctx context.Context, req *pb.UpdateInterfaceRequest) (*pbTypes.Interface, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return nil, nil
}

func (m *mockServer) UpdateRoutes(ctx context.Context, req *pb.UpdateRoutesRequest) (*pb.Routes, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return nil, nil
}

func (m *mockServer) OnlineCPUMem(ctx context.Context, req *pb.OnlineCPUMemRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}

func (m *mockServer) ListProcesses(ctx context.Context, req *pb.ListProcessesRequest) (*pb.ListProcessesResponse, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &pb.ListProcessesResponse{}, nil
}

func (m *mockServer) UpdateContainer(ctx context.Context, req *pb.UpdateContainerRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}
func (m *mockServer) StatsContainer(ctx context.Context, req *pb.StatsContainerRequest) (*pb.StatsContainerResponse, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &pb.StatsContainerResponse{}, nil

}

func (m *mockServer) PauseContainer(ctx context.Context, req *pb.PauseContainerRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}

func (m *mockServer) ResumeContainer(ctx context.Context, req *pb.ResumeContainerRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &types.Empty{}, nil
}

func (m *mockServer) ReseedRandomDev(ctx context.Context, req *pb.ReseedRandomDevRequest) (*types.Empty, error) {
	return &types.Empty{}, nil
}

func (m *mockServer) ListInterfaces(ctx context.Context, req *pb.ListInterfacesRequest) (*pb.Interfaces, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return nil, nil
}

func (m *mockServer) ListRoutes(ctx context.Context, req *pb.ListRoutesRequest) (*pb.Routes, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return nil, nil
}

func (m *mockServer) GetGuestDetails(ctx context.Context, req *pb.GuestDetailsRequest) (*pb.GuestDetailsResponse, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return nil, nil
}

func (m *mockServer) MemHotplugByProbe(ctx context.Context, req *pb.MemHotplugByProbeRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return nil, nil
}

func (m *mockServer) SetGuestDateTime(ctx context.Context, req *pb.SetGuestDateTimeRequest) (*types.Empty, error) {
	return &types.Empty{}, nil
}

func (m *mockServer) CopyFile(ctx context.Context, req *pb.CopyFileRequest) (*types.Empty, error) {
	mockLock.RLock()
	defer mockLock.RUnlock()
	return nil, m.podExist()
}

func (m *mockServer) StartTracing(ctx context.Context, req *pb.StartTracingRequest) (*types.Empty, error) {
	return nil, nil
}

func (m *mockServer) StopTracing(ctx context.Context, req *pb.StopTracingRequest) (*types.Empty, error) {
	return nil, nil
}
