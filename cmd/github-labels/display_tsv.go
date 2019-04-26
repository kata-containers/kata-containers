// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/csv"
	"os"
)

type displayTSV struct {
	writer *csv.Writer
}

func NewDisplayTSV(file *os.File) DisplayHandler {
	tsv := &displayTSV{}
	tsv.writer = csv.NewWriter(file)

	// Tab separator
	tsv.writer.Comma = rune('\t')

	return tsv
}

func (d *displayTSV) DisplayLabels(lf *LabelsFile) error {
	record := labelHeaderRecord()
	if err := d.writer.Write(record); err != nil {
		return err
	}

	for _, l := range lf.Labels {
		record := labelToRecord(l, false)

		if err := d.writer.Write(record); err != nil {
			return err
		}
	}

	d.writer.Flush()

	return d.writer.Error()
}

func (d *displayTSV) DisplayCategories(lf *LabelsFile, showLabels bool) error {
	record := categoryHeaderRecord(showLabels)
	if err := d.writer.Write(record); err != nil {
		return err
	}

	for _, c := range lf.Categories {
		record, err := categoryToRecord(lf, c, showLabels, false)
		if err != nil {
			return err
		}

		if err := d.writer.Write(record); err != nil {
			return err
		}
	}

	d.writer.Flush()

	return d.writer.Error()
}
