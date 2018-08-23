// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"

	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

// cnm is a network implementation for the CNM plugin.
type cnm struct {
	ctx context.Context
}

func (n *cnm) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "cnm")
}

func (n *cnm) trace(name string) (opentracing.Span, context.Context) {
	if n.ctx == nil {
		n.Logger().WithField("type", "bug").Error("trace called before context set")
		n.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(n.ctx, name)

	span.SetTag("subsystem", "network")
	span.SetTag("type", "cnm")

	return span, ctx
}

// init initializes the network, setting a new network namespace for the CNM network.
func (n *cnm) init(ctx context.Context, config NetworkConfig) (string, bool, error) {
	// Set context
	n.ctx = ctx

	span, _ := n.trace("init")
	defer span.Finish()

	return initNetworkCommon(config)
}

// run runs a callback in the specified network namespace.
func (n *cnm) run(networkNSPath string, cb func() error) error {
	span, _ := n.trace("run")
	defer span.Finish()

	return runNetworkCommon(networkNSPath, cb)
}

// add adds all needed interfaces inside the network namespace for the CNM network.
func (n *cnm) add(sandbox *Sandbox, config NetworkConfig, netNsPath string, netNsCreated bool) (NetworkNamespace, error) {
	span, _ := n.trace("add")
	defer span.Finish()

	endpoints, err := createEndpointsFromScan(netNsPath, config)
	if err != nil {
		return NetworkNamespace{}, err
	}

	networkNS := NetworkNamespace{
		NetNsPath:    netNsPath,
		NetNsCreated: netNsCreated,
		Endpoints:    endpoints,
	}

	if err := addNetworkCommon(sandbox, &networkNS); err != nil {
		return NetworkNamespace{}, err
	}

	return networkNS, nil
}

// remove network endpoints in the network namespace. It also deletes the network
// namespace in case the namespace has been created by us.
func (n *cnm) remove(sandbox *Sandbox, networkNS NetworkNamespace, netNsCreated bool) error {
	// Set the context again.
	//
	// This is required since when deleting networks, the init() method is
	// not called since the network config state is simply read from disk.
	// However, the context part of that state is not stored fully since
	// context.Context is an interface type meaning all the trace metadata
	// stored in the on-disk network config file is missing.
	n.ctx = sandbox.ctx

	span, _ := n.trace("remove")
	defer span.Finish()

	if err := removeNetworkCommon(networkNS, netNsCreated); err != nil {
		return err
	}

	if netNsCreated {
		return deleteNetNS(networkNS.NetNsPath)
	}

	return nil
}
