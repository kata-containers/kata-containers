// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// gRPC mock server

package mockserver

import (
	"errors"
	"fmt"

	google_protobuf2 "github.com/golang/protobuf/ptypes/empty"
	"golang.org/x/net/context"
	"google.golang.org/grpc"

	pb "github.com/kata-containers/agent/protocols/grpc"
)

const podStartingPid = 100

type pod struct {
	nextPid    uint32
	containers map[string]*container
}

// container init process pid is always
type container struct {
	id      string
	initPid uint32
	proc    map[uint32]*process
}

type process struct {
	pid  uint32
	proc *pb.Process
}

type mockServer struct {
	pod *pod
}

func NewMockServer() *grpc.Server {
	mock := &mockServer{}
	serv := grpc.NewServer()
	pb.RegisterAgentServiceServer(serv, mock)

	return serv
}

func validateOCISpec(spec *pb.Spec) error {
	if spec == nil || spec.Process == nil {
		return errors.New("invalid container spec")
	}
	return nil
}

func (m *mockServer) nextPid() uint32 {
	pid := m.pod.nextPid
	m.pod.nextPid += 1
	return pid
}

func (m *mockServer) checkExist(containerId string, pid uint32, createContainer, checkProcess bool) error {
	if m.pod == nil {
		return errors.New("pod not created")
	}
	if containerId == "" {
		return errors.New("container ID must be set")
	}
	if checkProcess && pid == 0 {
		return errors.New("process ID must be set")
	}

	// Check container existence
	if createContainer {
		if m.pod.containers[containerId] != nil {
			return fmt.Errorf("container ID %s already taken", containerId)
		}
		return nil
	} else if m.pod.containers[containerId] == nil {
		return fmt.Errorf("container %s does not exist", containerId)
	}

	// Check process existence
	if checkProcess {
		c := m.pod.containers[containerId]
		if c.proc[pid] == nil {
			return fmt.Errorf("process %d does not exist", pid)
		}
	}

	return nil
}

func (m *mockServer) processExist(containerId string, pid uint32) error {
	return m.checkExist(containerId, pid, false, true)
}

func (m *mockServer) containerExist(containerId string) error {
	return m.checkExist(containerId, 0, false, false)
}

func (m *mockServer) containerNonExist(containerId string) error {
	return m.checkExist(containerId, 0, true, false)
}

func (m *mockServer) podExist() error {
	if m.pod == nil {
		return errors.New("pod not created")
	}
	return nil
}

func (m *mockServer) CreateContainer(ctx context.Context, req *pb.CreateContainerRequest) (*google_protobuf2.Empty, error) {
	if err := m.containerNonExist(req.ContainerId); err != nil {
		return nil, err
	}

	if err := validateOCISpec(req.OCI); err != nil {
		return nil, err
	}

	c := &container{
		id:   req.ContainerId,
		proc: make(map[uint32]*process),
	}
	c.initPid = m.nextPid()
	c.proc[c.initPid] = &process{
		pid:  c.initPid,
		proc: req.OCI.Process,
	}
	m.pod.containers[req.ContainerId] = c

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) StartContainer(ctx context.Context, req *pb.StartContainerRequest) (*pb.NewProcessResponse, error) {
	if err := m.containerExist(req.ContainerId); err != nil {
		return nil, err
	}

	return &pb.NewProcessResponse{PID: m.pod.containers[req.ContainerId].initPid}, nil
}

func (m *mockServer) RemoveContainer(ctx context.Context, req *pb.RemoveContainerRequest) (*google_protobuf2.Empty, error) {
	if err := m.containerExist(req.ContainerId); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) ExecProcess(ctx context.Context, req *pb.ExecProcessRequest) (*pb.NewProcessResponse, error) {
	if err := m.containerExist(req.ContainerId); err != nil {
		return nil, err
	}

	c := m.pod.containers[req.ContainerId]
	pid := m.nextPid()
	c.proc[pid] = &process{
		pid:  pid,
		proc: req.Process,
	}
	return &pb.NewProcessResponse{PID: pid}, nil
}

func (m *mockServer) SignalProcess(ctx context.Context, req *pb.SignalProcessRequest) (*google_protobuf2.Empty, error) {
	if err := m.processExist(req.ContainerId, req.PID); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) WaitProcess(ctx context.Context, req *pb.WaitProcessRequest) (*pb.WaitProcessResponse, error) {
	if err := m.processExist(req.ContainerId, req.PID); err != nil {
		return nil, err
	}

	// remove process once it is waited
	c := m.pod.containers[req.ContainerId]
	c.proc[req.PID] = nil
	// container gone, clean it up
	if c.initPid == req.PID {
		m.pod.containers[req.ContainerId] = nil
	}

	return &pb.WaitProcessResponse{Status: 0}, nil
}

func (m *mockServer) WriteStdin(ctx context.Context, req *pb.WriteStreamRequest) (*pb.WriteStreamResponse, error) {
	if err := m.processExist(req.ContainerId, req.PID); err != nil {
		return nil, err
	}

	return &pb.WriteStreamResponse{Len: uint32(len(req.Data))}, nil
}

func (m *mockServer) ReadStdout(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	if err := m.processExist(req.ContainerId, req.PID); err != nil {
		return nil, err
	}

	return &pb.ReadStreamResponse{}, nil
}

func (m *mockServer) ReadStderr(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	if err := m.processExist(req.ContainerId, req.PID); err != nil {
		return nil, err
	}

	return &pb.ReadStreamResponse{}, nil
}

func (m *mockServer) CloseStdin(ctx context.Context, req *pb.CloseStdinRequest) (*google_protobuf2.Empty, error) {
	if err := m.processExist(req.ContainerId, req.PID); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) TtyWinResize(ctx context.Context, req *pb.TtyWinResizeRequest) (*google_protobuf2.Empty, error) {
	if err := m.processExist(req.ContainerId, req.PID); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) CreateSandbox(ctx context.Context, req *pb.CreateSandboxRequest) (*google_protobuf2.Empty, error) {
	if m.pod != nil {
		return nil, errors.New("pod already created")
	}
	m.pod = &pod{
		nextPid:    podStartingPid,
		containers: make(map[string]*container),
	}
	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) DestroySandbox(ctx context.Context, req *pb.DestroySandboxRequest) (*google_protobuf2.Empty, error) {
	if err := m.podExist(); err != nil {
		return nil, err
	}

	m.pod = nil
	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) AddInterface(context.Context, *pb.AddInterfaceRequest) (*google_protobuf2.Empty, error) {
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) RemoveInterface(context.Context, *pb.RemoveInterfaceRequest) (*google_protobuf2.Empty, error) {
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) RemoveRoute(context.Context, *pb.RouteRequest) (*google_protobuf2.Empty, error) {
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) UpdateInterface(ctx context.Context, req *pb.UpdateInterfaceRequest) (*google_protobuf2.Empty, error) {
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) AddRoute(ctx context.Context, req *pb.RouteRequest) (*google_protobuf2.Empty, error) {
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}

func (m *mockServer) OnlineCPUMem(ctx context.Context, req *pb.OnlineCPUMemRequest) (*google_protobuf2.Empty, error) {
	if err := m.podExist(); err != nil {
		return nil, err
	}

	return &google_protobuf2.Empty{}, nil
}
