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
	writer        *csv.Writer
	disableHeader bool
}

func NewDisplayTSV(file *os.File, separator rune, disableHeader bool) DisplayHandler {
	tsv := &displayTSV{
		disableHeader: disableHeader,
	}

	tsv.writer = csv.NewWriter(file)

	tsv.writer.Comma = separator

	return tsv
}

func (d *displayTSV) DisplayLinks(doc *Doc) error {
	if !d.disableHeader {
		record := linkHeaderRecord()
		if err := d.writer.Write(record); err != nil {
			return err
		}
	}

	for _, linkList := range doc.Links {
		for _, link := range linkList {
			record := linkToRecord(link)

			if err := d.writer.Write(record); err != nil {
				return err
			}
		}
	}

	d.writer.Flush()

	return d.writer.Error()
}

func (d *displayTSV) DisplayHeadings(doc *Doc) error {
	if !d.disableHeader {
		record := headingHeaderRecord()
		if err := d.writer.Write(record); err != nil {
			return err
		}
	}

	for _, l := range doc.Headings {
		record := headingToRecord(l)

		if err := d.writer.Write(record); err != nil {
			return err
		}
	}

	d.writer.Flush()

	return d.writer.Error()
}
