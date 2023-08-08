// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"github.com/montanaflynn/stats"
	log "github.com/sirupsen/logrus"
)

type statistics struct {
	Results     []float64 // Result array converted to floats
	Iterations  int       // How many results did we gather
	Mean        float64   // The 'average'
	Min         float64   // Smallest value we saw
	Max         float64   // Largest value we saw
	Range       float64   // Max - Min
	RangeSpread float64   // (Range/Min) * 100
	SD          float64   // Standard Deviation
	CoV         float64   // Co-efficient of Variation
}

// metrics represents the repository under test
// The members are Public so the toml reflection can see them, but I quite
// like the lower case toml naming, hence we use the annotation strings to
// get the parser to look for lower case.
type metrics struct {
	// Generic to JSON files
	// Generally mandatory
	Name        string `toml:"name"` //Used to locate the results file
	Description string `toml:"description"`

	// Optional config entries
	Type string `toml:"type"` //Default is JSON

	// Processing related entries
	CheckType string `toml:"checktype"` //Result val to calculate: mean, median, min, max
	// default: mean
	CheckVar string `toml:"checkvar"` //JSON: which var to (extract and) calculate on
	// is a 'jq' query

	stats statistics // collection of our stats data, calculated from Results

	// For setting 'bounds', you can either set a min/max value pair,
	// or you can set a mid-range value and a 'percentage gap'.
	// You should set one or the other. Setting both will likely result
	// in one of them being chosen first.

	// The range we expect the processed result to fall within
	// (MinVal <= Result <= MaxVal) == pass
	MinVal float64 `toml:"minval"`
	MaxVal float64 `toml:"maxval"`

	// If we are doing a percentage range check then you need to set
	// both a mid-value and a percentage range to check.
	MidVal     float64 `toml:"midval"`
	MinPercent float64 `toml:"minpercent"`
	MaxPercent float64 `toml:"maxpercent"`

	// Vars that are not in the toml file, but are filled out later
	// dynamically
	Gap float64 // What is the % gap between the Min and Max vals
}

// Calculate the statistics from the stored Results data
// Although the calculations can fail, we don't fail the function
func (m *metrics) calculate() {
	// First we check/calculate some non-stats values to fill out
	// our base data.

	// We should either have a Min/Max value pair or a percentage/MidVal
	// set. If we find a non-0 percentage set, then calculate the Min/Max
	// values from them, as the rest of the code base works off the Min/Max
	// values.
	if (m.MinPercent + m.MaxPercent) != 0 {
		m.MinVal = m.MidVal * (1 - (m.MinPercent / 100))
		m.MaxVal = m.MidVal * (1 + (m.MaxPercent / 100))

		// The rest of the system works off the Min/Max value
		// pair - so, if your min/max percentage values are not equal
		// then **the values you see in the results table will not look
		// like the ones you put in the toml file**, because they are
		// based off the mid-value calculation below.
		// This is unfortunate, but it keeps the code simpler overall.
	}

	// the gap is the % swing around the midpoint.
	midpoint := (m.MinVal + m.MaxVal) / 2
	m.Gap = (((m.MaxVal / midpoint) - 1) * 2) * 100

	// And now we work out the actual stats
	m.stats.Iterations = len(m.stats.Results)
	m.stats.Mean, _ = stats.Mean(m.stats.Results)
	m.stats.Min, _ = stats.Min(m.stats.Results)
	m.stats.Max, _ = stats.Max(m.stats.Results)
	m.stats.Range = m.stats.Max - m.stats.Min
	m.stats.RangeSpread = (m.stats.Range / m.stats.Min) * 100.0
	m.stats.SD, _ = stats.StandardDeviation(m.stats.Results)
	m.stats.CoV = (m.stats.SD / m.stats.Mean) * 100.0

	log.Debugf(" Iters is %d", m.stats.Iterations)
	log.Debugf(" Min is %f", m.stats.Min)
	log.Debugf(" Max is %f", m.stats.Max)
	log.Debugf(" Mean is %f", m.stats.Mean)
	log.Debugf(" SD is %f", m.stats.SD)
	log.Debugf(" CoV is %.2f", m.stats.CoV)
}
