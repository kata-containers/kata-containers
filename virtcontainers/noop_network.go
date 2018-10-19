// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

// noopNetwork a.k.a. NO-OP Network is an empty network implementation, for
// testing and mocking purposes.
type noopNetwork struct {
}

// run runs a callback in the specified network namespace for
// the Noop network.
// It does nothing.
func (n *noopNetwork) run(networkNSPath string, cb func() error) error {
	return cb()
}

// add adds all needed interfaces inside the network namespace the Noop network.
// It does nothing.
func (n *noopNetwork) add(sandbox *Sandbox, hotattach bool) error {
	return nil
}

// remove unbridges and deletes TAP interfaces. It also removes virtual network
// interfaces and deletes the network namespace for the Noop network.
// It does nothing.
func (n *noopNetwork) remove(sandbox *Sandbox, hotdetach bool) error {
	return nil
}
