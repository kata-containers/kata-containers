// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
)

type ccShim struct{}

// start is the ccShim start implementation.
// It starts the cc-shim binary with URL and token flags provided by
// the proxy.
func (s *ccShim) start(sandbox *Sandbox, params ShimParams) (int, error) {
	if sandbox.config == nil {
		return -1, fmt.Errorf("Sandbox config cannot be nil")
	}

	config, ok := newShimConfig(*(sandbox.config)).(ShimConfig)
	if !ok {
		return -1, fmt.Errorf("Wrong shim config type, should be CCShimConfig type")
	}

	if config.Path == "" {
		return -1, fmt.Errorf("Shim path cannot be empty")
	}

	if params.Token == "" {
		return -1, fmt.Errorf("Token cannot be empty")
	}

	if params.URL == "" {
		return -1, fmt.Errorf("URL cannot be empty")
	}

	if params.Container == "" {
		return -1, fmt.Errorf("Container cannot be empty")
	}

	args := []string{config.Path, "-c", params.Container, "-t", params.Token, "-u", params.URL}
	if config.Debug {
		args = append(args, "-d")
	}

	return startShim(args, params)
}
