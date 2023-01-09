//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
)

type displayText struct {
}

func (d *displayText) Display(entries *LogEntries, fieldNames []string, file *os.File) error {
	if err := addCommentHeader(fieldNames, file); err != nil {
		return err
	}

	for i, entry := range entries.Entries {
		fmt.Fprintf(file, "Record %d: %+v\n", 1+i, entry)
	}

	return nil
}
