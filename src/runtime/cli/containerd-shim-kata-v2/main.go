// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"

	"github.com/containerd/containerd/runtime/v2/shim"
	"github.com/kata-containers/kata-containers/src/runtime/containerd-shim-v2"
)

const shim_id = "io.containerd.kata.v2"

func shimConfig(config *shim.Config) {
	config.NoReaper = true
	config.NoSubreaper = true
}

func main() {

	if len(os.Args) == 2 && os.Args[1] == "--version" {
		fmt.Printf("%s containerd shim: id: %q, version: %s, commit: %v\n", project, shim_id, version, commit)
		os.Exit(0)
	}

	shim.Run(shim_id, containerdshim.New, shimConfig)
}
