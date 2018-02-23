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

// noopNetwork a.k.a. NO-OP Network is an empty network implementation, for
// testing and mocking purposes.
type noopNetwork struct {
}

// init initializes the network, setting a new network namespace for the Noop network.
// It does nothing.
func (n *noopNetwork) init(config NetworkConfig) (string, bool, error) {
	return "", true, nil
}

// run runs a callback in the specified network namespace for
// the Noop network.
// It does nothing.
func (n *noopNetwork) run(networkNSPath string, cb func() error) error {
	return cb()
}

// add adds all needed interfaces inside the network namespace the Noop network.
// It does nothing.
func (n *noopNetwork) add(pod Pod, config NetworkConfig, netNsPath string, netNsCreated bool) (NetworkNamespace, error) {
	return NetworkNamespace{}, nil
}

// remove unbridges and deletes TAP interfaces. It also removes virtual network
// interfaces and deletes the network namespace for the Noop network.
// It does nothing.
func (n *noopNetwork) remove(pod Pod, networkNS NetworkNamespace) error {
	return nil
}
