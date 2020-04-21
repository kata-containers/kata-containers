// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
// +build !go1.10
//
// The go library only got math.Round in go1.10. If we are building
// with a version of Go older than that then add our own Round routine
// Code derived directly from the code/example at:
// https://github.com/golang/go/blob/master/src/math/floor.go
//
// Note, there is a twin round_post9_test.go file for go>=1.10 versions
// that does a callthrough to the standard library version

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
	t := math.Trunc(x)
	if math.Abs(x-t) >= 0.5 {
		return t + math.Copysign(1, x)
	}
	return t
}
