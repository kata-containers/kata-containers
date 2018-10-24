// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"

	"github.com/containernetworking/plugins/pkg/ns"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

type defNetwork struct {
}

func (n *defNetwork) logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "default-network")
}

func (n *defNetwork) trace(ctx context.Context, name string) (opentracing.Span, context.Context) {
	span, ct := opentracing.StartSpanFromContext(ctx, name)

	span.SetTag("subsystem", "network")
	span.SetTag("type", "default")

	return span, ct
}

// run runs a callback in the specified network namespace.
func (n *defNetwork) run(networkNSPath string, cb func() error) error {
	span, _ := n.trace(context.Background(), "run")
	defer span.Finish()

	return doNetNS(networkNSPath, func(_ ns.NetNS) error {
		return cb()
	})
}

// add adds all needed interfaces inside the network namespace.
func (n *defNetwork) add(s *Sandbox) error {
	span, _ := n.trace(s.ctx, "add")
	defer span.Finish()

	endpoints, err := createEndpointsFromScan(s.config.NetworkConfig.NetNSPath, s.config.NetworkConfig)
	if err != nil {
		return err
	}

	s.networkNS.Endpoints = endpoints

	err = doNetNS(s.config.NetworkConfig.NetNSPath, func(_ ns.NetNS) error {
		for _, endpoint := range s.networkNS.Endpoints {
			n.logger().WithField("endpoint-type", endpoint.Type()).Info("Attaching endpoint")
			if err := endpoint.Attach(s.hypervisor); err != nil {
				return err
			}
		}

		return nil
	})
	if err != nil {
		return err
	}

	n.logger().Debug("Network added")

	return nil
}

// remove network endpoints in the network namespace. It also deletes the network
// namespace in case the namespace has been created by us.
func (n *defNetwork) remove(s *Sandbox) error {
	span, _ := n.trace(s.ctx, "remove")
	defer span.Finish()

	for _, endpoint := range s.networkNS.Endpoints {
		// Detach for an endpoint should enter the network namespace
		// if required.
		n.logger().WithField("endpoint-type", endpoint.Type()).Info("Detaching endpoint")
		if err := endpoint.Detach(s.networkNS.NetNsCreated, s.networkNS.NetNsPath); err != nil {
			return err
		}
	}

	n.logger().Debug("Network removed")

	if s.networkNS.NetNsCreated {
		n.logger().Infof("Network namespace %q deleted", s.networkNS.NetNsPath)
		return deleteNetNS(s.networkNS.NetNsPath)
	}

	return nil
}
