// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"math"
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

	// We do a little funky math on Gap to round it to within 0.1% - as the actual
	// gap math gave us 10.000000000000009 ...
	roundedGap := math.Round(m.Gap/0.001) * 0.001
	assert.Equal(10.0, roundedGap, "Should be equal")
	assert.Equal(2.0, m.stats.Mean, "Should be equal")
	assert.Equal(1.0, m.stats.Min, "Should be equal")
	assert.Equal(3.0, m.stats.Max, "Should be equal")
	assert.Equal(2.0, m.stats.Range, "Should be equal")
	assert.Equal(200.0, m.stats.RangeSpread, "Should be equal")
	assert.Equal(0.816496580927726, m.stats.SD, "Should be equal")
	assert.Equal(40.8248290463863, m.stats.CoV, "Should be equal")
}

// Test that only setting a % range works
func TestCalculate2(t *testing.T) {
	assert := assert.New(t)

	var m = metrics{
		Name:        "name",
		Description: "desc",
		Type:        "type",
		CheckType:   "json",
		CheckVar:    "Results",
		//MinVal:    1.9,
		//MaxVal:    2.1,
		MinPercent: 20,
		MaxPercent: 25,
		MidVal:     2.0,
		Gap:        0,
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

	// We do a little funky math on Gap to round it to within 0.1% - as the actual
	// gap math gave us 10.000000000000009 ...
	roundedGap := math.Round(m.Gap/0.001) * 0.001
	// This is not a nice (20+25), as the 'midval' will skew it.
	assert.Equal(43.902, roundedGap, "Should be equal")
	assert.Equal(2.0, m.stats.Mean, "Should be equal")
	assert.Equal(1.0, m.stats.Min, "Should be equal")
	assert.Equal(3.0, m.stats.Max, "Should be equal")
	assert.Equal(2.0, m.stats.Range, "Should be equal")
	assert.Equal(200.0, m.stats.RangeSpread, "Should be equal")
	assert.Equal(0.816496580927726, m.stats.SD, "Should be equal")
	assert.Equal(40.8248290463863, m.stats.CoV, "Should be equal")
}
