// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	log "github.com/Sirupsen/logrus"
	"github.com/montanaflynn/stats"
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

	// The range we expect the processed result to fall within
	// (MinVal <= Result <= MaxVal) == pass
	MinVal float64 `toml:"minval"`
	MaxVal float64 `toml:"maxval"`

	// Vars that are not in the toml file, but are filled out later
	// dynamically
	Gap float64 // What is the % gap between the Min and Max vals

	stats statistics // collection of our stats data, calculated from Results
}

// Calculate the statistics from the stored Results data
// Although the calculations can fail, we don't fail the function
func (m *metrics) calculate() {

	midpoint := (m.MinVal + m.MaxVal) / 2
	// the gap is the % swing around the midpoint.
	m.Gap = (((m.MaxVal / midpoint) - 1) * 2) * 100
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
