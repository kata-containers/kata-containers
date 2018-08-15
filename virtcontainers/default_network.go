// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"

	"github.com/containernetworking/plugins/pkg/ns"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

type defNetwork struct {
	ctx context.Context
}

func (n *defNetwork) logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "default-network")
}

func (n *defNetwork) trace(name string) (opentracing.Span, context.Context) {
	if n.ctx == nil {
		n.logger().WithField("type", "bug").Error("trace called before context set")
		n.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(n.ctx, name)

	span.SetTag("subsystem", "network")
	span.SetTag("type", "default")

	return span, ctx
}

// init initializes the network, setting a new network namespace.
func (n *defNetwork) init(ctx context.Context, config NetworkConfig) (string, bool, error) {
	// Set context
	n.ctx = ctx

	span, _ := n.trace("init")
	defer span.Finish()

	if !config.InterworkingModel.IsValid() || config.InterworkingModel == NetXConnectDefaultModel {
		config.InterworkingModel = DefaultNetInterworkingModel
	}

	if config.NetNSPath == "" {
		path, err := createNetNS()
		if err != nil {
			return "", false, err
		}

		return path, true, nil
	}

	isHostNs, err := hostNetworkingRequested(config.NetNSPath)
	if err != nil {
		return "", false, err
	}

	if isHostNs {
		return "", false, fmt.Errorf("Host networking requested, not supported by runtime")
	}

	return config.NetNSPath, false, nil
}

// run runs a callback in the specified network namespace.
func (n *defNetwork) run(networkNSPath string, cb func() error) error {
	span, _ := n.trace("run")
	defer span.Finish()

	if networkNSPath == "" {
		return fmt.Errorf("networkNSPath cannot be empty")
	}

	return doNetNS(networkNSPath, func(_ ns.NetNS) error {
		return cb()
	})
}

// add adds all needed interfaces inside the network namespace.
func (n *defNetwork) add(sandbox *Sandbox, config NetworkConfig, netNsPath string, netNsCreated bool) (NetworkNamespace, error) {
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

	err = doNetNS(networkNS.NetNsPath, func(_ ns.NetNS) error {
		for _, endpoint := range networkNS.Endpoints {
			if err := endpoint.Attach(sandbox.hypervisor); err != nil {
				return err
			}
		}

		return nil
	})
	if err != nil {
		return NetworkNamespace{}, err
	}

	n.logger().Debug("Network added")

	return networkNS, nil
}

// remove network endpoints in the network namespace. It also deletes the network
// namespace in case the namespace has been created by us.
func (n *defNetwork) remove(sandbox *Sandbox, networkNS NetworkNamespace, netNsCreated bool) error {
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

	for _, endpoint := range networkNS.Endpoints {
		// Detach for an endpoint should enter the network namespace
		// if required.
		if err := endpoint.Detach(netNsCreated, networkNS.NetNsPath); err != nil {
			return err
		}
	}

	n.logger().Debug("Network removed")

	if netNsCreated {
		n.logger().Infof("Network namespace %q deleted", networkNS.NetNsPath)
		return deleteNetNS(networkNS.NetNsPath)
	}

	return nil
}
