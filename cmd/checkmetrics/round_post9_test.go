// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
// +build go1.10
//
// If we have math.Round() available, then use it. It was only added
// in go1.10.
//
// Note, there is a twin round_pre10_test.go file for go<1.10 versions
// that implements a Round function.

package main

import (
	"math"
)

// Round returns the nearest integer, rounding half away from zero.
//
// Special cases are:
//	Round(±0) = ±0
//	Round(±Inf) = ±Inf
//	Round(NaN) = NaN
func Round(x float64) float64 {
	return math.Round(x)
}
