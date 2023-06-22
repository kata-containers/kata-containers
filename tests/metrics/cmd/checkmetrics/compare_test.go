// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"math"
	"testing"

	"github.com/stretchr/testify/assert"
)

// Pre-filled out metrics (apart from the calculated stats)
// This should **pass** the "mean" metrics checks by default
var exampleM = metrics{
	Name:        "name",
	Description: "desc",
	Type:        "type",
	CheckType:   "json",
	CheckVar:    "Results",
	MinVal:      0.9,
	MaxVal:      3.1,
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

func TestGenSummaryLine(t *testing.T) {

	assert := assert.New(t)

	var args = []string{
		"name",
		"minval",
		"mean",
		"maxval",
		"gap",
		"min",
		"max",
		"rnge",
		"cov",
		"iterations"}

	// Check for the 'passed' case
	s := (&metricsCheck{}).genSummaryLine(
		true,    //passed
		args[0], //name
		args[1], //minval
		args[2], //mean
		args[3], //maxval
		args[4], //gap
		args[5], //min
		args[6], //max
		args[7], //rnge
		args[8], //cov
		args[9]) //iterations

	for n, i := range s {
		if n == 0 {
			assert.Equal("P", i, "Should be equal")
		} else {
			assert.Equal(args[n-1], i, "Should be equal")
		}
	}

	// Check for the 'failed' case
	s = (&metricsCheck{}).genSummaryLine(
		false,   //passed
		args[0], //name
		args[1], //minval
		args[2], //mean
		args[3], //maxval
		args[4], //gap
		args[5], //min
		args[6], //max
		args[7], //rnge
		args[8], //cov
		args[9]) //iterations

	for n, i := range s {
		if n == 0 {
			assert.Equal("*F*", i, "Should be equal")
		} else {
			assert.Equal(args[n-1], i, "Should be equal")
		}
	}
}

func TestCheckStats(t *testing.T) {
	assert := assert.New(t)

	var m = exampleM
	m.Name = "CheckStats"

	//Check before we have done the calculations - should fail
	_, err := (&metricsCheck{}).checkstats(m)
	assert.Error(err)

	m.calculate()

	// Constants here calculated from info coded in struct above

	// Funky rounding of Gap, as float imprecision actually gives us
	// 110.00000000000001 - check to within 0.1% then...
	roundedGap := math.Round(m.Gap/0.001) * 0.001
	assert.Equal(110.0, roundedGap, "Should be equal")
	assert.Equal(2.0, m.stats.Mean, "Should be equal")
	assert.Equal(1.0, m.stats.Min, "Should be equal")
	assert.Equal(3.0, m.stats.Max, "Should be equal")
	assert.Equal(2.0, m.stats.Range, "Should be equal")
	assert.Equal(200.0, m.stats.RangeSpread, "Should be equal")
	assert.Equal(0.816496580927726, m.stats.SD, "Should be equal")
	assert.Equal(40.8248290463863, m.stats.CoV, "Should be equal")

	s, err := (&metricsCheck{}).checkstats(m)
	assert.NoError(err)

	assert.Equal("P", s[0], "Should be equal")          // Pass
	assert.Equal("CheckStats", s[1], "Should be equal") // test name
	assert.Equal("0.90", s[2], "Should be equal")       // Floor
	assert.Equal("2.00", s[3], "Should be equal")       // Mean
	assert.Equal("3.10", s[4], "Should be equal")       // Ceiling
	assert.Equal("110.0%", s[5], "Should be equal")     // Gap
	assert.Equal("1.00", s[6], "Should be equal")       // Min
	assert.Equal("3.00", s[7], "Should be equal")       // Max
	assert.Equal("200.0%", s[8], "Should be equal")     // Range %
	assert.Equal("40.8%", s[9], "Should be equal")      // CoV
	assert.Equal("3", s[10], "Should be equal")         // Iterations

	// And check in percentage presentation mode
	showPercentage = true
	s, err = (&metricsCheck{}).checkstats(m)
	assert.NoError(err)

	assert.Equal("P", s[0], "Should be equal")          // Pass
	assert.Equal("CheckStats", s[1], "Should be equal") // test name
	assert.Equal("45.0%", s[2], "Should be equal")      // Floor
	assert.Equal("100.0%", s[3], "Should be equal")     // Mean
	assert.Equal("155.0%", s[4], "Should be equal")     // Ceiling
	assert.Equal("110.0%", s[5], "Should be equal")     // Gap
	assert.Equal("50.0%", s[6], "Should be equal")      // Min
	assert.Equal("150.0%", s[7], "Should be equal")     // Max
	assert.Equal("200.0%", s[8], "Should be equal")     // Range %
	assert.Equal("40.8%", s[9], "Should be equal")      // CoV
	assert.Equal("3", s[10], "Should be equal")         // Iterations

	// And put the default back
	showPercentage = false

	// Funcs called with a Min that fails and a Max that fails
	// Presumption is that unmodified metrics should pass

	// FIXME - we don't test the actual < vs <= boudary type conditions

	// Mean is 2.0
	CheckMean(assert, 3.0, 1.0)

	// Min is 1.0
	CheckMin(assert, 3.0, 0.5)

	// Max is 3.0
	CheckMax(assert, 4.0, 1.0)

	// CoV is 40.8
	CheckCoV(assert, 50.0, 1.0)

	// SD is 0.8165
	CheckSD(assert, 1.0, 0.5)
}

func CheckMean(assert *assert.Assertions, badmin float64, badmax float64) {
	m := exampleM
	m.CheckType = "mean"
	m.Name = "CheckMean"
	// Do the stats
	m.calculate()

	// Defaults should pass
	_, err := (&metricsCheck{}).checkstats(m)
	assert.NoError(err)

	// badmin should fail
	old := m.MinVal
	m.MinVal = badmin
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
	m.MinVal = old

	// badmax should fail
	m.MaxVal = badmax
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
}

func CheckMin(assert *assert.Assertions, badmin float64, badmax float64) {

	m := exampleM
	m.CheckType = "min"
	m.Name = "CheckMin"
	// Do the stats
	m.calculate()

	// Defaults should pass
	_, err := (&metricsCheck{}).checkstats(m)
	assert.NoError(err)

	// badmin should fail
	old := m.MinVal
	m.MinVal = badmin
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
	m.MinVal = old

	// badmax should fail
	m.MaxVal = badmax
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
}

func CheckMax(assert *assert.Assertions, badmin float64, badmax float64) {
	m := exampleM
	m.CheckType = "max"
	m.Name = "CheckMax"
	// Do the stats
	m.calculate()

	// Defaults should pass
	_, err := (&metricsCheck{}).checkstats(m)
	assert.NoError(err)

	// badmin should fail
	old := m.MinVal
	m.MinVal = badmin
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
	m.MinVal = old

	// badmax should fail
	m.MaxVal = badmax
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
}

func CheckSD(assert *assert.Assertions, badmin float64, badmax float64) {
	m := exampleM
	m.CheckType = "sd"
	m.Name = "CheckSD"
	// Do the stats
	m.calculate()

	// Set it up to pass by default
	m.MinVal = 0.9 * m.stats.SD
	m.MaxVal = 1.1 * m.stats.SD

	oldMin := m.MinVal
	oldMax := m.MinVal

	// Defaults should pass
	_, err := (&metricsCheck{}).checkstats(m)
	assert.NoError(err)

	// badmin should fail
	m.MinVal = badmin
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
	m.MinVal = oldMin

	// badmax should fail
	m.MaxVal = badmax
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
	m.MaxVal = oldMax
}

func CheckCoV(assert *assert.Assertions, badmin float64, badmax float64) {
	m := exampleM
	m.CheckType = "cov"
	m.Name = "CheckCoV"
	// Do the stats
	m.calculate()

	// Set it up to pass by default
	m.MinVal = 0.9 * m.stats.CoV
	m.MaxVal = 1.1 * m.stats.CoV

	oldMin := m.MinVal
	oldMax := m.MinVal

	// Defaults should pass
	_, err := (&metricsCheck{}).checkstats(m)
	assert.NoError(err)

	// badmin should fail
	m.MinVal = badmin
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
	m.MinVal = oldMin

	// badmax should fail
	m.MaxVal = badmax
	_, err = (&metricsCheck{}).checkstats(m)
	assert.Error(err)
	m.MaxVal = oldMax
}
