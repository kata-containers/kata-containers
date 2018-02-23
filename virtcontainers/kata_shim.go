//
// Copyright (c) 2017 Intel Corporation
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
func (s *kataShim) start(pod Pod, params ShimParams) (int, error) {
	if pod.config == nil {
		return -1, fmt.Errorf("Pod config cannot be nil")
	}

	config, ok := newShimConfig(*(pod.config)).(ShimConfig)
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
