// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os/exec"

	"github.com/containernetworking/plugins/pkg/ns"
)

const shimNsPath = "/proc/self/ns/net"

func startInShimNS(cmd *exec.Cmd) error {
	// Create nydusd in shim netns as it needs to access host network
	return doNetNS(shimNsPath, func(_ ns.NetNS) error {
		return cmd.Start()
	})
}
