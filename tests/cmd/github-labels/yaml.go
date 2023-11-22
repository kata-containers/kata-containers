// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"sort"

	yaml "gopkg.in/yaml.v2"
)

const fileMode os.FileMode = 0600

func readYAML(file string) (*LabelsFile, error) {
	bytes, err := os.ReadFile(file)
	if err != nil {
		return nil, err
	}

	lf := LabelsFile{}

	err = yaml.Unmarshal(bytes, &lf)
	if err != nil {
		return nil, err
	}

	sort.Sort(lf.Labels)
	sort.Sort(lf.Categories)

	clean(&lf)

	err = check(&lf)
	if err != nil {
		return nil, fmt.Errorf("file was not in expected format: %v", err)
	}

	return &lf, nil
}

func writeYAML(lf *LabelsFile, file string) error {
	bytes, err := yaml.Marshal(lf)
	if err != nil {
		return err
	}

	return os.WriteFile(file, bytes, fileMode)
}

func checkYAML(file string) error {
	// read and check
	_, err := readYAML(file)

	if err == nil {
		fmt.Printf("Checked file %v\n", file)
	}

	return err
}

func sortYAML(fromFile, toFile string) error {
	// read and sort
	lf, err := readYAML(fromFile)
	if err != nil {
		return err
	}

	return writeYAML(lf, toFile)
}
