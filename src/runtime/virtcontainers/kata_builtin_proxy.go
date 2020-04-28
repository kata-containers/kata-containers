// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "fmt"

// This is a kata builtin proxy implementation of the proxy interface. Kata proxy
// functionality is implemented inside the virtcontainers library.
type kataBuiltInProxy struct {
	proxyBuiltin
}

func (p *kataBuiltInProxy) validateParams(params proxyParams) error {
	if len(params.id) == 0 || len(params.agentURL) == 0 || len(params.consoleURL) == 0 {
		return fmt.Errorf("Invalid proxy parameters %+v", params)
	}

	return nil
}

// start is the proxy start implementation for kata builtin proxy.
// It starts the console watcher for the guest.
// It returns agentURL to let agent connect directly.
func (p *kataBuiltInProxy) start(params proxyParams) (int, string, error) {
	if err := p.validateParams(params); err != nil {
		return -1, "", err
	}

	return p.proxyBuiltin.start(params)
}
