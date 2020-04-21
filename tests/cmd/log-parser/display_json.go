//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"os"
)

type displayJSON struct {
}

func (d *displayJSON) Display(entries *LogEntries, fieldNames []string, file *os.File) error {
	encoder := json.NewEncoder(file)

	encoder.SetIndent(displayPrefix, displayIndentValue)

	return encoder.Encode(entries)
}
