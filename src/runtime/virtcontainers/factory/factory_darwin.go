// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"context"

	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/pkg/errors"
)

var unsupportedFactory error = errors.New("VM factory is unsupported on Darwin")

type factory struct {
}

func NewFactory(ctx context.Context, config Config, fetchOnly bool) (vc.Factory, error) {
	return &factory{}, unsupportedFactory
}

func (f *factory) Config() vc.VMConfig {
	return vc.VMConfig{}
}

func (f *factory) GetVMStatus() []*pb.GrpcVMStatus {
	return nil
}

func (f *factory) GetVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	return nil, unsupportedFactory
}

func (f *factory) GetBaseVM(ctx context.Context, config vc.VMConfig) (*vc.VM, error) {
	return nil, unsupportedFactory
}

func (f *factory) CloseFactory(ctx context.Context) {
	return
}
