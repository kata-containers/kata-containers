// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/kata-containers/runtime/virtcontainers/pkg/mock"
)

func main() {
	config := mock.ShimMockConfig{
		Name:               "cc-shim",
		URLParamName:       "u",
		ContainerParamName: "c",
		TokenParamName:     "t",
	}

	mock.StartShim(config)
}
