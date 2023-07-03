// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"fmt"
	"os"

	"github.com/BurntSushi/toml"
	log "github.com/sirupsen/logrus"
)

type baseFile struct {
	// metrics is the slice of Metrics imported from the TOML config file
	Metric []metrics
}

// newBasefile imports the TOML file passed from the path passed in the file
// argument and returns the baseFile slice containing the import if successful
func newBasefile(file string) (*baseFile, error) {
	if file == "" {
		log.Error("Missing basefile argument")
		return nil, fmt.Errorf("missing baseline reference file")
	}

	configuration, err := os.ReadFile(file)
	if err != nil {
		return nil, err
	}

	var basefile baseFile
	if err := toml.Unmarshal(configuration, &basefile); err != nil {
		return nil, err
	}

	if len(basefile.Metric) == 0 {
		log.Warningf("No entries found in basefile [%s]\n", file)
	}

	return &basefile, nil
}
