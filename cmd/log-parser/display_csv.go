//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/csv"
	"fmt"
	"os"
	"reflect"
)

type displayCSV struct {
}

func (d *displayCSV) logEntryToSlice(entry LogEntry) []string {
	var record []string

	v := reflect.ValueOf(entry)

	for i := 0; i < v.NumField(); i++ {
		field := v.Field(i)

		value := fmt.Sprintf("%v", field.Interface())
		record = append(record, value)
	}

	return record
}

func (d *displayCSV) Display(entries *LogEntries, fieldNames []string, file *os.File) error {
	writer := csv.NewWriter(file)

	// header showing the format of the subsequent records
	if err := writer.Write(fieldNames); err != nil {
		return err
	}

	for _, entry := range entries.Entries {
		record := d.logEntryToSlice(entry)

		if err := writer.Write(record); err != nil {
			return err
		}
	}

	writer.Flush()

	return writer.Error()
}
