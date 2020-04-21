//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"strings"

	bf "gopkg.in/russross/blackfriday.v2"
)

// forceCreateHeadings extracts "missed" headings from the specified node,
// returning a slice of the newly headings created (which need to be added by the
// caller).
//
// Alas, Black Friday isn't 100% reliable...
func (d *Doc) forceCreateHeadings(node *bf.Node) ([]Heading, error) {
	if err := checkNode(node, bf.Text); err != nil {
		return []Heading{}, err
	}

	chunk := string(node.Literal)

	if chunk == "" {
		// No text in this node
		return []Heading{}, nil
	}

	lines := strings.Split(chunk, "\n")
	if len(lines) <= 1 {
		// No headings lurking in this text node
		return []Heading{}, nil
	}

	var headings []Heading

	for _, line := range lines {
		if !strings.HasPrefix(line, anchorPrefix) {
			continue
		}

		fields := strings.Split(line, anchorPrefix)
		name := strings.Join(fields, "")
		name = strings.TrimSpace(name)

		count := strings.Count(line, anchorPrefix)

		heading := Heading{
			Name:  name,
			Level: count,
		}

		id, err := createHeadingID(heading.Name)
		if err != nil {
			return []Heading{}, err
		}

		heading.LinkName = id

		headings = append(headings, heading)

		extraHeadings++
	}

	return headings, nil
}
