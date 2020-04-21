//
// Copyright (c) 2018 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"flag"

	"code.cloudfoundry.org/bytefmt"
)

var (
	argMemTotal    = flag.String("mem-total", "0", "total memory to be consumed. Memory will be consumed via multiple allocations.")
	argMemStepSize = flag.String("mem-alloc-size", "4K", "amount of memory to be consumed in each allocation")
	buffer         [][]byte
)

func main() {
	flag.Parse()
	total, _ := bytefmt.ToBytes(*argMemTotal)
	stepSize, _ := bytefmt.ToBytes(*argMemStepSize)
	allocateMemory(total, stepSize)
}

func allocateMemory(total, stepSize uint64) {
	for i := uint64(1); i*stepSize <= total; i++ {
		newBuffer := make([]byte, stepSize)
		for i := range newBuffer {
			newBuffer[i] = 0
		}
		buffer = append(buffer, newBuffer)
	}
}
