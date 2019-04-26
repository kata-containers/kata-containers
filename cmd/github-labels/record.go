// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"strings"
)

const (
	labelNamesSeparator = ","
)

func labelToRecord(l Label, quote bool) (record []string) {
	name := l.Name
	category := l.CategoryName
	colour := l.Colour
	from := l.From

	if quote {
		name = fmt.Sprintf("`%s`", l.Name)
		category = fmt.Sprintf("`%s`", l.CategoryName)
		colour = fmt.Sprintf("`%s`", l.Colour)
		if from != "" {
			from = fmt.Sprintf("`%s`", l.From)
		}
	}

	record = append(record, name)
	record = append(record, l.Description)
	record = append(record, category)
	record = append(record, colour)
	record = append(record, from)

	return record
}

func labelHeaderRecord() []string {
	return []string{
		"Name",
		"Description",
		"Category",
		"Colour",
		"From",
	}
}

func categoryHeaderRecord(showLabels bool) []string {
	var fields []string

	fields = append(fields, "Name")
	fields = append(fields, "Description")
	fields = append(fields, "URL")

	if showLabels {
		fields = append(fields, "Labels")
	}

	return fields
}

func categoryToRecord(lf *LabelsFile, c Category, showLabels, quote bool) ([]string, error) {
	var record []string

	name := c.Name

	if quote {
		name = fmt.Sprintf("`%s`", c.Name)
	}

	record = append(record, name)
	record = append(record, c.Description)
	record = append(record, c.URL)

	if showLabels {
		var labelNames []string

		labels, err := getLabelsByCategory(c.Name, lf)
		if err != nil {
			return nil, err
		}

		for _, l := range labels {
			labelName := l.Name

			if quote {
				labelName = fmt.Sprintf("`%s`", l.Name)
			}

			labelNames = append(labelNames, labelName)
		}

		result := strings.Join(labelNames, labelNamesSeparator)

		record = append(record, result)
	}

	return record, nil
}
