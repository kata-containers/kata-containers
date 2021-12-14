// Copyright (c) 2019 SUSE LLC
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"

	"github.com/pkg/errors"
	"gopkg.in/yaml.v2"
)

func yamlUnmarshal(yamlFile string, cfg interface{}) error {
	source, err := os.ReadFile(yamlFile)
	if err != nil {
		return err
	}
	err = yaml.Unmarshal(source, cfg)
	if err != nil {
		return errors.Wrapf(err, fmt.Sprintf("cannot unmarshal %s", yamlFile))
	}
	return nil
}
