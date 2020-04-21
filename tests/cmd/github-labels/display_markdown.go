// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os"

	"github.com/olekukonko/tablewriter"
)

type displayMD struct {
	writer *tablewriter.Table
}

func NewDisplayMD(file *os.File) DisplayHandler {
	md := &displayMD{}

	md.writer = tablewriter.NewWriter(file)
	md.writer.SetCenterSeparator("|")

	md.writer.SetBorders(tablewriter.Border{
		Left:   true,
		Right:  true,
		Top:    false,
		Bottom: false,
	})

	// Critical for GitHub Flavoured Markdown
	md.writer.SetAutoWrapText(false)

	return md
}

func (d *displayMD) render(headerFields []string, records [][]string) {
	d.writer.SetHeader(headerFields)
	d.writer.AppendBulk(records)
	d.writer.Render()
}

func (d *displayMD) DisplayLabels(lf *LabelsFile) error {
	var records [][]string

	for _, l := range lf.Labels {
		record := labelToRecord(l, true)
		records = append(records, record)
	}

	headerFields := labelHeaderRecord()

	d.render(headerFields, records)

	return nil
}

func (d *displayMD) DisplayCategories(lf *LabelsFile, showLabels bool) error {
	headerFields := categoryHeaderRecord(showLabels)

	var records [][]string

	for _, c := range lf.Categories {
		record, err := categoryToRecord(lf, c, showLabels, true)
		if err != nil {
			return err
		}

		records = append(records, record)
	}

	d.render(headerFields, records)

	return nil
}
