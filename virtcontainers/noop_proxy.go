// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

// This is a dummy proxy implementation of the proxy interface, only
// used for testing purpose.
type noopProxy struct{}

var noopProxyURL = "noopProxyURL"

// register is the proxy start implementation for testing purpose.
// It does nothing.
func (p *noopProxy) start(sandbox *Sandbox, params proxyParams) (int, string, error) {
	return 0, noopProxyURL, nil
}

// stop is the proxy stop implementation for testing purpose.
// It does nothing.
func (p *noopProxy) stop(sandbox *Sandbox, pid int) error {
	return nil
}

// The noopproxy doesn't need to watch the vm console, thus return false always.
func (p *noopProxy) consoleWatched() bool {
	return false
}
