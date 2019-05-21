//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"

	"github.com/Sirupsen/logrus"
)

// addHeading adds the specified heading to the document.
//
// Note that headings must be unique.
func (d *Doc) addHeading(heading Heading) error {
	name := heading.Name

	if name == "" {
		return d.Errorf("heading name cannot be blank: %+v", heading)
	}

	if heading.LinkName == "" {
		return d.Errorf("heading link name cannot be blank: %q (%+v)",
			name, heading)
	}

	if heading.Level <= 0 {
		return d.Errorf("heading level must be atleast 1: %q (%+v)",
			name, heading)
	}

	if _, ok := d.Headings[name]; ok {
		return d.Errorf("duplicate heading: %q (heading: %+v)",
			name, heading)
	}

	// Potentially change the ID to handle strange characters
	// supported in links by GitHub.
	id, err := createHeadingID(heading.Name)
	if err != nil {
		return err
	}

	heading.LinkName = id

	d.Logger.WithField("heading", fmt.Sprintf("%+v", heading)).Debug("adding heading")

	d.Headings[name] = heading

	return nil
}

// addLink potentially adds the specified link to the document.
//
// Note that links do not need to be unique: a document can contain
// multiple links with:
//
// - the same description and the same address.
// - the same description but with different addresses.
// - different descriptions but with the same address.
//
func (d *Doc) addLink(link Link) error {
	addr := link.Address

	if addr == "" {
		return d.Errorf("link address cannot be blank: %+v", link)
	}

	if link.Type == unknownLink {
		return d.Errorf("BUG: link type invalid: %+v", link)
	}

	// Not checked by default as magic "build status" / go report / godoc
	// links don't have a description - they have a image only.
	if strict && link.Description == "" {
		return d.Errorf("link description cannot be blank: %q (%+v)",
			addr, link)
	}

	fields := logrus.Fields{
		"link": fmt.Sprintf("%+v", link),
	}

	if _, ok := d.Links[addr]; ok {
		d.Logger.WithFields(fields).Debug("not adding duplicate link")

		return nil
	}

	d.Logger.WithFields(fields).Debug("adding link")

	d.Links[addr] = link

	return nil
}
