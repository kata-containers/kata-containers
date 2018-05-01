// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

type noopShim struct{}

// start is the noopShim start implementation for testing purpose.
// It does nothing.
func (s *noopShim) start(sandbox *Sandbox, params ShimParams) (int, error) {
	return 0, nil
}
