// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"

	containerdtypes "github.com/containerd/containerd/api/types"
	shimapi "github.com/containerd/containerd/runtime/v2/shim"
	"google.golang.org/protobuf/proto"

	shim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/types"
)

func shimConfig(config *shimapi.Config) {
	config.NoReaper = true
	config.NoSubreaper = true
}

func handleInfoFlag() {
	info := &containerdtypes.RuntimeInfo{
		Name: types.DefaultKataRuntimeName,
		Version: &containerdtypes.RuntimeVersion{
			Version:  katautils.VERSION,
			Revision: katautils.COMMIT,
		},
	}

	data, err := proto.Marshal(info)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to marshal RuntimeInfo: %v\n", err)
		os.Exit(1)
	}

	os.Stdout.Write(data)
	os.Exit(0)
}

func main() {

	if len(os.Args) == 2 && os.Args[1] == "--version" {
		fmt.Printf("%s containerd shim (Golang): id: %q, version: %s, commit: %v\n", katautils.PROJECT, types.DefaultKataRuntimeName, katautils.VERSION, katautils.COMMIT)
		os.Exit(0)
	}

	if len(os.Args) == 2 && os.Args[1] == "-info" {
		handleInfoFlag()
	}

	shimapi.Run(types.DefaultKataRuntimeName, shim.New, shimConfig)
}
