//
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

import (
	"github.com/sirupsen/logrus"
)

// cnm is a network implementation for the CNM plugin.
type cnm struct {
	config NetworkConfig
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
func (n *cnm) add(pod Pod, config NetworkConfig, netNsPath string, netNsCreated bool) (NetworkNamespace, error) {
	endpoints, err := createEndpointsFromScan(netNsPath, config)
	if err != nil {
		return NetworkNamespace{}, err
	}

	networkNS := NetworkNamespace{
		NetNsPath:    netNsPath,
		NetNsCreated: netNsCreated,
		Endpoints:    endpoints,
	}

	if err := addNetworkCommon(pod, &networkNS); err != nil {
		return NetworkNamespace{}, err
	}

	return networkNS, nil
}

// remove unbridges and deletes TAP interfaces. It also removes virtual network
// interfaces and deletes the network namespace for the CNM network.
func (n *cnm) remove(pod Pod, networkNS NetworkNamespace) error {
	if err := removeNetworkCommon(networkNS); err != nil {
		return err
	}

	return deleteNetNS(networkNS.NetNsPath, true)
}
