//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"strings"

	bf "gopkg.in/russross/blackfriday.v2"
)

// displayTOC displays a table of contents entry for the specified node.
func (d *Doc) displayTOC(node *bf.Node) error {
	switch node.Type {
	case bf.Heading:
		return d.displayTOCEntryFromNode(node)
	case bf.Text:
		// handle blackfriday deficiencies
		headings, err := d.forceCreateHeadings(node)
		if err != nil {
			return err
		}

		for _, heading := range headings {
			err := d.displayTOCEntryFromHeading(heading)
			if err != nil {
				return err
			}
		}
	}

	return nil
}

// displayTOCEntryFromHeading displays a table of contents entry
// for the specified heading.
func (d *Doc) displayTOCEntryFromHeading(heading Heading) error {
	const indentSpaces = 4

	prefix := ""

	level := heading.Level

	// Indent needs to be zero for top level headings
	level--

	if level > 0 {
		prefix = strings.Repeat(" ", level*indentSpaces)
	}

	entry := fmt.Sprintf("[%s](%s%s)", heading.MDName, anchorPrefix, heading.LinkName)

	fmt.Printf("%s%s %s\n", prefix, listPrefix, entry)

	return nil
}

// displayTOCEntryFromHeading displays a table of contents entry
// for the specified heading.
func (d *Doc) displayTOCEntryFromNode(node *bf.Node) error {
	if err := checkNode(node, bf.Heading); err != nil {
		return err
	}

	heading, err := d.makeHeading(node)
	if err != nil {
		return err
	}

	return d.displayTOCEntryFromHeading(heading)
}
