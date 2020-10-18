// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"

	"github.com/containerd/containerd/runtime/v2/shim"
	containerdshim "github.com/kata-containers/kata-containers/src/runtime/containerd-shim-v2"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/types"
)

func shimConfig(config *shim.Config) {
	config.NoReaper = true
	config.NoSubreaper = true
}

func main() {

	if len(os.Args) == 2 && os.Args[1] == "--version" {
		fmt.Printf("%s containerd shim: id: %q, version: %s, commit: %v\n", project, types.DefaultKataRuntimeName, version, commit)
		os.Exit(0)
	}

	shim.Run(types.DefaultKataRuntimeName, containerdshim.New, shimConfig)
}
