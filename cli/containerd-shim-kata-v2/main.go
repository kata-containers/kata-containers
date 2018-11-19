// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/containerd/containerd/runtime/v2/shim"
	"github.com/kata-containers/runtime/containerd-shim-v2"
)

func main() {
	shim.Run("io.containerd.kata.v2", containerdshim.New)
}
