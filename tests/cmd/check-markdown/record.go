// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "fmt"

func linkHeaderRecord() []string {
	return []string{
		"Document",
		"Address",
		"Path",
		"Description",
		"Type",
	}
}

func linkToRecord(l Link) (record []string) {
	record = append(record, l.Doc.Name)
	record = append(record, l.Address)
	record = append(record, l.ResolvedPath)
	record = append(record, l.Description)
	record = append(record, l.Type.String())

	return record
}

func headingHeaderRecord() []string {
	return []string{
		"Name",
		"Link",
		"Level",
	}
}
func headingToRecord(h Heading) (record []string) {
	record = append(record, h.Name)
	record = append(record, h.LinkName)
	record = append(record, fmt.Sprintf("%d", h.Level))

	return record
}
