// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package mock

import (
	"context"
	"fmt"
	"io/ioutil"
	"net"
	"net/url"

	"github.com/containerd/ttrpc"
	gpb "github.com/gogo/protobuf/types"
	aTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	pb "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
)

var testKataMockHybridVSockURLTempl = "mock://%s/kata-mock-hybrid-vsock.sock"

func GenerateKataMockHybridVSock() (string, error) {
	dir, err := ioutil.TempDir("", "kata-mock-hybrid-vsock-test")
	if err != nil {
		return "", err
	}

	return fmt.Sprintf(testKataMockHybridVSockURLTempl, dir), nil
}

// HybridVSockTTRPCMock is the ttrpc-based mock hybrid-vsock backend implementation
type HybridVSockTTRPCMock struct {
	// HybridVSockTTRPCMockImp is the structure implementing
	// the ttrpc interface we want the mock hybrid-vsock server to serve.
	HybridVSockTTRPCMockImp

	listener net.Listener
}

func (hv *HybridVSockTTRPCMock) ttrpcRegister(s *ttrpc.Server) {
	pb.RegisterAgentServiceService(s, &hv.HybridVSockTTRPCMockImp)
	pb.RegisterHealthService(s, &hv.HybridVSockTTRPCMockImp)
}

// Start starts the ttrpc-based mock hybrid-vsock server
func (hv *HybridVSockTTRPCMock) Start(socketAddr string) error {
	if socketAddr == "" {
		return fmt.Errorf("Missing Socket Address")
	}

	url, err := url.Parse(socketAddr)
	if err != nil {
		return err
	}

	l, err := net.Listen("unix", url.Path)
	if err != nil {
		return err
	}

	hv.listener = l

	ttrpcServer, err := ttrpc.NewServer()
	if err != nil {
		return err
	}
	hv.ttrpcRegister(ttrpcServer)

	go func() {
		ttrpcServer.Serve(context.Background(), l)
	}()

	return nil
}

// Stop stops the ttrpc-based mock hybrid-vsock server
func (hv *HybridVSockTTRPCMock) Stop() error {
	if hv.listener == nil {
		return fmt.Errorf("Missing mock hvbrid vsock listener")
	}

	return hv.listener.Close()
}

type HybridVSockTTRPCMockImp struct{}

var emptyResp = &gpb.Empty{}

func (p *HybridVSockTTRPCMockImp) CreateContainer(ctx context.Context, req *pb.CreateContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) StartContainer(ctx context.Context, req *pb.StartContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) ExecProcess(ctx context.Context, req *pb.ExecProcessRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) SignalProcess(ctx context.Context, req *pb.SignalProcessRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) WaitProcess(ctx context.Context, req *pb.WaitProcessRequest) (*pb.WaitProcessResponse, error) {
	return &pb.WaitProcessResponse{}, nil
}

func (p *HybridVSockTTRPCMockImp) UpdateContainer(ctx context.Context, req *pb.UpdateContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) RemoveContainer(ctx context.Context, req *pb.RemoveContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) WriteStdin(ctx context.Context, req *pb.WriteStreamRequest) (*pb.WriteStreamResponse, error) {
	return &pb.WriteStreamResponse{}, nil
}

func (p *HybridVSockTTRPCMockImp) ReadStdout(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	return &pb.ReadStreamResponse{}, nil
}

func (p *HybridVSockTTRPCMockImp) ReadStderr(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	return &pb.ReadStreamResponse{}, nil
}

func (p *HybridVSockTTRPCMockImp) CloseStdin(ctx context.Context, req *pb.CloseStdinRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) TtyWinResize(ctx context.Context, req *pb.TtyWinResizeRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) CreateSandbox(ctx context.Context, req *pb.CreateSandboxRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) DestroySandbox(ctx context.Context, req *pb.DestroySandboxRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) UpdateInterface(ctx context.Context, req *pb.UpdateInterfaceRequest) (*aTypes.Interface, error) {
	return &aTypes.Interface{}, nil
}

func (p *HybridVSockTTRPCMockImp) UpdateRoutes(ctx context.Context, req *pb.UpdateRoutesRequest) (*pb.Routes, error) {
	return &pb.Routes{}, nil
}

func (p *HybridVSockTTRPCMockImp) ListInterfaces(ctx context.Context, req *pb.ListInterfacesRequest) (*pb.Interfaces, error) {
	return &pb.Interfaces{}, nil
}

func (p *HybridVSockTTRPCMockImp) ListRoutes(ctx context.Context, req *pb.ListRoutesRequest) (*pb.Routes, error) {
	return &pb.Routes{}, nil
}

func (p *HybridVSockTTRPCMockImp) AddARPNeighbors(ctx context.Context, req *pb.AddARPNeighborsRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) OnlineCPUMem(ctx context.Context, req *pb.OnlineCPUMemRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) StatsContainer(ctx context.Context, req *pb.StatsContainerRequest) (*pb.StatsContainerResponse, error) {
	return &pb.StatsContainerResponse{}, nil
}

func (p *HybridVSockTTRPCMockImp) Check(ctx context.Context, req *pb.CheckRequest) (*pb.HealthCheckResponse, error) {
	return &pb.HealthCheckResponse{}, nil
}

func (p *HybridVSockTTRPCMockImp) Version(ctx context.Context, req *pb.CheckRequest) (*pb.VersionCheckResponse, error) {
	return &pb.VersionCheckResponse{}, nil
}

func (p *HybridVSockTTRPCMockImp) PauseContainer(ctx context.Context, req *pb.PauseContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) ResumeContainer(ctx context.Context, req *pb.ResumeContainerRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) ReseedRandomDev(ctx context.Context, req *pb.ReseedRandomDevRequest) (*gpb.Empty, error) {
	return emptyResp, nil
}

func (p *HybridVSockTTRPCMockImp) GetGuestDetails(ctx context.Context, req *pb.GuestDetailsRequest) (*pb.GuestDetailsResponse, error) {
	return &pb.GuestDetailsResponse{}, nil
}

func (p *HybridVSockTTRPCMockImp) SetGuestDateTime(ctx context.Context, req *pb.SetGuestDateTimeRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *HybridVSockTTRPCMockImp) CopyFile(ctx context.Context, req *pb.CopyFileRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *HybridVSockTTRPCMockImp) StartTracing(ctx context.Context, req *pb.StartTracingRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *HybridVSockTTRPCMockImp) StopTracing(ctx context.Context, req *pb.StopTracingRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *HybridVSockTTRPCMockImp) MemHotplugByProbe(ctx context.Context, req *pb.MemHotplugByProbeRequest) (*gpb.Empty, error) {
	return &gpb.Empty{}, nil
}

func (p *HybridVSockTTRPCMockImp) GetOOMEvent(ctx context.Context, req *pb.GetOOMEventRequest) (*pb.OOMEvent, error) {
	return &pb.OOMEvent{}, nil
}

func (p *HybridVSockTTRPCMockImp) GetMetrics(ctx context.Context, req *pb.GetMetricsRequest) (*pb.Metrics, error) {
	return &pb.Metrics{}, nil
}
