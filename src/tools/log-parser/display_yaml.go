//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"

	yaml "gopkg.in/yaml.v2"
)

type displayYAML struct {
}

func (d *displayYAML) Display(entries *LogEntries, fieldNames []string, file *os.File) error {
	bytes, err := yaml.Marshal(entries)
	if err != nil {
		return err
	}

	if err = addCommentHeader(fieldNames, file); err != nil {
		return err
	}

	_, err = fmt.Fprintln(file, string(bytes))

	return err
}
