// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"io"

	context "golang.org/x/net/context"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"

	pb "github.com/kata-containers/agent/protocols/grpc"
)

type inPipe struct {
	ctx         context.Context
	agent       *shimAgent
	containerId string
	pid         uint32
}

func (p *inPipe) Write(data []byte) (n int, err error) {
	resp, err := p.agent.WriteStdin(p.ctx, &pb.WriteStreamRequest{
		ContainerId: p.containerId,
		PID:         p.pid,
		Data:        data})
	if err != nil {
		return 0, err
	}

	return int(resp.Len), nil
}

func (p *inPipe) Close() error {
	_, err := p.agent.CloseStdin(p.ctx, &pb.CloseStdinRequest{
		ContainerId: p.containerId,
		PID:         p.pid})

	return err
}

type readFn func(context.Context, *pb.ReadStreamRequest, ...grpc.CallOption) (*pb.ReadStreamResponse, error)

func pipeRead(ctx context.Context, containerId string, pid uint32, data []byte, read readFn) (n int, err error) {
	resp, err := read(ctx, &pb.ReadStreamRequest{
		ContainerId: containerId,
		PID:         pid,
		Len:         uint32(len(data))})
	if err == nil {
		copy(data, resp.Data)
		return len(resp.Data), nil
	}

	// check if it is &grpc.rpcError{code:0xb, desc:"EOF"} and return io.EOF instead
	if grpc.Code(err) == codes.OutOfRange && grpc.ErrorDesc(err) == "EOF" {
		return 0, io.EOF
	}
	return 0, err
}

type outPipe struct {
	ctx         context.Context
	agent       *shimAgent
	containerId string
	pid         uint32
}

func (p *outPipe) Read(data []byte) (n int, err error) {
	return pipeRead(p.ctx, p.containerId, p.pid, data, p.agent.ReadStdout)
}

type errPipe struct {
	ctx         context.Context
	agent       *shimAgent
	containerId string
	pid         uint32
}

func (p *errPipe) Read(data []byte) (n int, err error) {
	return pipeRead(p.ctx, p.containerId, p.pid, data, p.agent.ReadStderr)
}

func shimStdioPipe(ctx context.Context, agent *shimAgent, containerId string, pid uint32) (io.WriteCloser, io.Reader, io.Reader) {
	return &inPipe{ctx: ctx, agent: agent, containerId: containerId, pid: pid},
		&outPipe{ctx: ctx, agent: agent, containerId: containerId, pid: pid}, &errPipe{ctx: ctx, agent: agent, containerId: containerId, pid: pid}
}
