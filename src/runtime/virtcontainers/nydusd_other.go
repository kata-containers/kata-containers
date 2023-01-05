// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

//go:build !linux

package virtcontainers

import "os/exec"

// No-op on net namespace join on other platforms.
func startInShimNS(cmd *exec.Cmd) error {
	return cmd.Start()
}
