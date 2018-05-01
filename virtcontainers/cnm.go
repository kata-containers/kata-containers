// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"github.com/sirupsen/logrus"
)

// cnm is a network implementation for the CNM plugin.
type cnm struct {
}

func cnmLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "cnm")
}

// init initializes the network, setting a new network namespace for the CNM network.
func (n *cnm) init(config NetworkConfig) (string, bool, error) {
	return initNetworkCommon(config)
}

// run runs a callback in the specified network namespace.
func (n *cnm) run(networkNSPath string, cb func() error) error {
	return runNetworkCommon(networkNSPath, cb)
}

// add adds all needed interfaces inside the network namespace for the CNM network.
func (n *cnm) add(sandbox *Sandbox, config NetworkConfig, netNsPath string, netNsCreated bool) (NetworkNamespace, error) {
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

// remove unbridges and deletes TAP interfaces. It also removes virtual network
// interfaces and deletes the network namespace for the CNM network.
func (n *cnm) remove(sandbox *Sandbox, networkNS NetworkNamespace) error {
	if err := removeNetworkCommon(networkNS); err != nil {
		return err
	}

	return deleteNetNS(networkNS.NetNsPath, true)
}
