// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

type kataBuiltInShim struct{}

// start is the kataBuiltInShim start implementation for kata builtin shim.
// It does nothing. The shim functionality is provided by the virtcontainers
// library.
func (s *kataBuiltInShim) start(sandbox *Sandbox, params ShimParams) (int, error) {
	return -1, nil
}
