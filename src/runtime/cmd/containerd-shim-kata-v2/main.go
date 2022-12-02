// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"

	shimapi "github.com/containerd/containerd/runtime/v2/shim"

	shim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/types"
)

func shimConfig(config *shimapi.Config) {
	config.NoReaper = true
	config.NoSubreaper = true
}

func main() {

	if len(os.Args) == 2 && os.Args[1] == "--version" {
		fmt.Printf("%s containerd shim (Golang): id: %q, version: %s, commit: %v\n", katautils.PROJECT, types.DefaultKataRuntimeName, katautils.VERSION, katautils.COMMIT)
		os.Exit(0)
	}

	shimapi.Run(types.DefaultKataRuntimeName, shim.New, shimConfig)
}
