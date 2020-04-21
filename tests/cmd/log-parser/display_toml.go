//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os"

	"github.com/BurntSushi/toml"
)

type displayTOML struct {
}

func (d *displayTOML) Display(entries *LogEntries, fieldNames []string, file *os.File) error {
	encoder := toml.NewEncoder(file)

	encoder.Indent = displayIndentValue

	if err := addCommentHeader(fieldNames, file); err != nil {
		return err
	}

	return encoder.Encode(entries)
}
