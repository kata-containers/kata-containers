// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
)

type kataShim struct{}

// KataShimConfig is the structure providing specific configuration
// for kataShim implementation.
type KataShimConfig struct {
	Path  string
	Debug bool
}

// start is the ccShim start implementation.
// It starts the cc-shim binary with URL and token flags provided by
// the proxy.
func (s *kataShim) start(sandbox *Sandbox, params ShimParams) (int, error) {
	if sandbox.config == nil {
		return -1, fmt.Errorf("Sandbox config cannot be nil")
	}

	config, ok := newShimConfig(*(sandbox.config)).(ShimConfig)
	if !ok {
		return -1, fmt.Errorf("Wrong shim config type, should be KataShimConfig type")
	}

	if config.Path == "" {
		return -1, fmt.Errorf("Shim path cannot be empty")
	}

	if params.URL == "" {
		return -1, fmt.Errorf("URL cannot be empty")
	}

	if params.Container == "" {
		return -1, fmt.Errorf("Container cannot be empty")
	}

	if params.Token == "" {
		return -1, fmt.Errorf("Process token cannot be empty")
	}

	args := []string{config.Path, "-agent", params.URL, "-container", params.Container, "-exec-id", params.Token}

	if params.Terminal {
		args = append(args, "-terminal")
	}

	if config.Debug {
		args = append(args, "-log", "debug")
	}

	return startShim(args, params)
}
