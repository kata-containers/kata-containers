// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

type kernelModule struct {
	// maps parameter names to values
	parameters map[string]string

	// description
	desc string

	// if it is definitely required
	required bool
}

type vmContainerCapableDetails struct {
	requiredCPUFlags      map[string]string
	requiredCPUAttribs    map[string]string
	requiredKernelModules map[string]kernelModule
	cpuInfoFile           string
}
