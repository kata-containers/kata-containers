// Copyright (c) 2019 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// grpccache implements base vm factory that get base vm from grpc

package grpccache

import (
	"context"
	"fmt"

	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/base"
	"github.com/pkg/errors"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	emptypb "google.golang.org/protobuf/types/known/emptypb"
)

type grpccache struct {
	conn   *grpc.ClientConn
	config *vc.VMConfig
}

// New returns a new direct vm factory.
func New(ctx context.Context, endpoint string) (base.FactoryBase, error) {
	conn, err := grpc.Dial(fmt.Sprintf("unix://%s", endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, errors.Wrapf(err, "failed to connect %q", endpoint)
	}

	jConfig, err := pb.NewCacheServiceClient(conn).Config(ctx, &emptypb.Empty{})
	if err != nil {
		return nil, errors.Wrapf(err, "failed to Config")
	}

	config, err := vc.GrpcToVMConfig(jConfig)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to convert JSON to VMConfig")
	}

	return &grpccache{conn: conn, config: config}, nil
}

// Config returns the direct factory's configuration.
func (g *grpccache) Config() vc.VMConfig {
	return *g.config
}

// GetBaseVM create a new VM directly.
func (g *grpccache) GetBaseVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	defer g.conn.Close()
	gVM, err := pb.NewCacheServiceClient(g.conn).GetBaseVM(ctx, &emptypb.Empty{})
	if err != nil {
		return nil, errors.Wrapf(err, "failed to GetBaseVM")
	}
	return vc.NewVMFromGrpc(ctx, gVM, *g.config)
}

// CloseFactory closes the direct vm factory.
func (g *grpccache) CloseFactory(ctx context.Context) {
}

// GetVMStatus is not supported
func (g *grpccache) GetVMStatus() []*pb.GrpcVMStatus {
	panic("ERROR: package grpccache does not support GetVMStatus")
}
