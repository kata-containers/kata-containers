// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestCalculate(t *testing.T) {
	assert := assert.New(t)

	var m = metrics{
		Name:        "name",
		Description: "desc",
		Type:        "type",
		CheckType:   "json",
		CheckVar:    "Results",
		MinVal:      1.9,
		MaxVal:      2.1,
		Gap:         0,
		stats: statistics{
			Results:     []float64{1.0, 2.0, 3.0},
			Iterations:  3,
			Mean:        0.0,
			Min:         0.0,
			Max:         0.0,
			Range:       0.0,
			RangeSpread: 0.0,
			SD:          0.0,
			CoV:         0.0}}

	m.calculate()

	// Constants here calculated from info coded in struct above
	assert.Equal(10.5263157894737, m.Gap, "Should be equal")
	assert.Equal(2.0, m.stats.Mean, "Should be equal")
	assert.Equal(1.0, m.stats.Min, "Should be equal")
	assert.Equal(3.0, m.stats.Max, "Should be equal")
	assert.Equal(2.0, m.stats.Range, "Should be equal")
	assert.Equal(200.0, m.stats.RangeSpread, "Should be equal")
	assert.Equal(0.816496580927726, m.stats.SD, "Should be equal")
	assert.Equal(40.8248290463863, m.stats.CoV, "Should be equal")
}
