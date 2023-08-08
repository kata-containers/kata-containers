// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"errors"
	"strconv"

	log "github.com/sirupsen/logrus"
)

// metricsCheck is a placeholder struct for us to attach the methods to and make
// it clear they belong this grouping. Maybe there is a better way?
type metricsCheck struct {
}

// reportTitleSlice returns the report table title row as a slice of strings
func (mc *metricsCheck) reportTitleSlice() []string {

	// FIXME - now we don't only check the mean, let's re-arrange the order
	// to make a little more sense.
	// Also, CoV is so much more useful than SD - let's stop printout out
	// the SD, and add instead the % gap between the Min and Max Results
	return []string{"P/F",
		"Name",
		// This is the check boundary, not the smallest value in Results
		"Flr",
		"Mean",
		// This is the check boundary, not the largest value in Results
		"Ceil",
		"Gap",
		"Min",
		"Max",
		"Rng",
		"Cov",
		"Its"}
}

// genSummaryLine takes in all the relevant report arguments and returns
// a string slice formatted appropriately for the summary table generation
func (mc *metricsCheck) genSummaryLine(
	passed bool,
	name string,
	minval string,
	mean string,
	maxval string,
	gap string,
	min string,
	max string,
	rnge string,
	cov string,
	iterations string) (summary []string) {

	if passed {
		summary = append(summary, "P")
	} else {
		summary = append(summary, "*F*")
	}

	summary = append(summary,
		name,
		minval,
		mean,
		maxval,
		gap,
		min,
		max,
		rnge,
		cov,
		iterations)

	return
}

// genErrorLine takes a number of error argument strings and a pass/fail bool
// and returns a string slice formatted appropriately for the summary report.
// It exists to hide some of the inner details of just how the slice is meant
// to be formatted, such as the exact number of columns
func (mc *metricsCheck) genErrorLine(
	passed bool,
	error1 string,
	error2 string,
	error3 string) (summary []string) {

	summary = mc.genSummaryLine(passed, error1, error2, error3,
		"", "", "", "", "", "", "")
	return
}

// check takes a basefile metric record and a filled out stats struct and checks
// if the file metrics pass the metrics comparison checks.
// check returns a string slice containing the results of the check.
// The err return will be non-nil if the check fails.
func (mc *metricsCheck) checkstats(m metrics) (summary []string, err error) {
	var pass = true
	var val float64

	log.Debugf("Compare check for [%s]", m.Name)

	log.Debugf("Checking value [%s]", m.CheckType)

	//Pick out the value we are range checking depending on the
	// config. Default if not set is the "mean"
	switch m.CheckType {
	case "min":
		val = m.stats.Min

	case "max":
		val = m.stats.Max

	case "cov":
		val = m.stats.CoV

	case "sd":
		val = m.stats.SD

	case "mean":
		fallthrough
	default:
		val = m.stats.Mean
	}

	log.Debugf(" Check minval (%f < %f)", m.MinVal, val)
	if val < m.MinVal {
		log.Warnf("Failed Minval (%7f > %7f) for [%s]",
			m.MinVal, val,
			m.Name)
		pass = false
	} else {
		log.Debug("Passed")
	}

	log.Debugf(" Check maxval (%f > %f)", m.MaxVal, val)
	if val > m.MaxVal {
		log.Warnf("Failed Maxval (%7f < %7f) for [%s]",
			m.MaxVal, val,
			m.Name)
		pass = false
	} else {
		log.Debug("Passed")
	}

	if !pass {
		err = errors.New("Failed")
	}

	// Note - choosing the precision for the fields is tricky without
	// knowledge of the actual metrics tests results. For now set
	// precision to 'probably big enough', and later we may want to
	// add an annotation to the TOML baselines to give an indication of
	// expected values - or, maybe we can derive it from the min/max values

	// Are we presenting as a percentage based difference
	if showPercentage {
		// Work out what our midpoint baseline 'goal' is.
		midpoint := (m.MinVal + m.MaxVal) / 2

		// Calculate our values as a % based off the mid-point
		// of the acceptable range.
		floorpc := (m.MinVal / midpoint) * 100.0
		ceilpc := (m.MaxVal / midpoint) * 100.0
		meanpc := (m.stats.Mean / midpoint) * 100.0
		minpc := (m.stats.Min / midpoint) * 100.0
		maxpc := (m.stats.Max / midpoint) * 100.0

		// Or present as physical values
		summary = append(summary, mc.genSummaryLine(
			pass,
			m.Name,
			// Note this is the check boundary, not the smallest Result seen
			strconv.FormatFloat(floorpc, 'f', 1, 64)+"%",
			strconv.FormatFloat(meanpc, 'f', 1, 64)+"%",
			// Note this is the check boundary, not the largest Result seen
			strconv.FormatFloat(ceilpc, 'f', 1, 64)+"%",
			strconv.FormatFloat(m.Gap, 'f', 1, 64)+"%",
			strconv.FormatFloat(minpc, 'f', 1, 64)+"%",
			strconv.FormatFloat(maxpc, 'f', 1, 64)+"%",
			strconv.FormatFloat(m.stats.RangeSpread, 'f', 1, 64)+"%",
			strconv.FormatFloat(m.stats.CoV, 'f', 1, 64)+"%",
			strconv.Itoa(m.stats.Iterations))...)
	} else {
		// Or present as physical values
		summary = append(summary, mc.genSummaryLine(
			pass,
			m.Name,
			// Note this is the check boundary, not the smallest Result seen
			strconv.FormatFloat(m.MinVal, 'f', 2, 64),
			strconv.FormatFloat(m.stats.Mean, 'f', 2, 64),
			// Note this is the check boundary, not the largest Result seen
			strconv.FormatFloat(m.MaxVal, 'f', 2, 64),
			strconv.FormatFloat(m.Gap, 'f', 1, 64)+"%",
			strconv.FormatFloat(m.stats.Min, 'f', 2, 64),
			strconv.FormatFloat(m.stats.Max, 'f', 2, 64),
			strconv.FormatFloat(m.stats.RangeSpread, 'f', 1, 64)+"%",
			strconv.FormatFloat(m.stats.CoV, 'f', 1, 64)+"%",
			strconv.Itoa(m.stats.Iterations))...)
	}

	return
}
