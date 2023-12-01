//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

// headingByLinkName returns the heading associated with the specified link name.
func (d *Doc) headingByLinkName(linkName string) *Heading {
	for _, heading := range d.Headings {
		if heading.LinkName == linkName {
			return &heading
		}
	}

	return nil
}

// heading returns the heading with the name specified.
func (d *Doc) heading(name string) *Heading {
	for _, heading := range d.Headings {
		if name == heading.LinkName {
			return &heading
		}
	}

	return nil
}
