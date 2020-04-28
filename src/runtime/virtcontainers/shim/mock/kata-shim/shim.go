// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/kata-containers/runtime/virtcontainers/pkg/mock"
)

func main() {
	config := mock.ShimMockConfig{
		Name:               "kata-shim",
		URLParamName:       "agent",
		ContainerParamName: "container",
		TokenParamName:     "exec-id",
	}

	mock.StartShim(config)
}
