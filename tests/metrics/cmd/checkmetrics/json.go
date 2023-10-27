// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"bufio"
	"bytes"
	"io"
	"os/exec"
	"strconv"

	log "github.com/sirupsen/logrus"
)

// jsonRecord has no data - the data is loaded and processed and stored
// back into the metrics structure passed in.
type jsonRecord struct {
}

// load reads in a JSON 'Metrics' results file from the file path given
// Parse out the actual results data using the 'jq' query found in the
// respective TOML entry.
func (c *jsonRecord) load(filepath string, metric *metrics) error {
	var err error

	log.Debugf("in json load of [%s]", filepath)

	log.Debugf(" Run jq '%v' %s", metric.CheckVar, filepath)

	out, err := exec.Command("jq", "-r", metric.CheckVar, filepath).Output()
	if err != nil {
		log.Warnf("Failed to run [jq %v %v][%v]", metric.CheckVar, filepath, err)
		return err
	}

	log.Debugf(" Got result [%v]", out)

	// Try to parse the results as floats first...
	floats, err := readFloats(bytes.NewReader(out))

	if err != nil {
		// And if they are not floats, check if they are ints...
		ints, err := readInts(bytes.NewReader(out))

		if err != nil {
			log.Warnf("Failed to decode [%v]", out)
			return err
		}

		// Always store the internal data as floats
		floats = []float64{}
		for _, i := range ints {
			floats = append(floats, float64(i))
		}
	}

	log.Debugf(" and got output [%v]", floats)

	// Store the results back 'up'
	metric.stats.Results = floats
	// And do the stats on them
	metric.calculate()

	return nil
}

// Parse a string of ascii ints into a slice of ints
func readInts(r io.Reader) ([]int, error) {
	scanner := bufio.NewScanner(r)
	scanner.Split(bufio.ScanWords)
	var result []int
	for scanner.Scan() {
		i, err := strconv.Atoi(scanner.Text())
		if err != nil {
			return result, err
		}
		result = append(result, i)
	}
	return result, scanner.Err()
}

// Parse a string of ascii floats into a slice of floats
func readFloats(r io.Reader) ([]float64, error) {
	scanner := bufio.NewScanner(r)
	scanner.Split(bufio.ScanWords)
	var result []float64
	for scanner.Scan() {
		f, err := strconv.ParseFloat(scanner.Text(), 64)
		if err != nil {
			return result, err
		}
		result = append(result, f)
	}
	return result, scanner.Err()
}
