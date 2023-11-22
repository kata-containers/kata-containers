//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"
)

// checkLink checks the validity of the specified link. If checkOtherDoc is
// true and the link is an external one, validate the link by considering the
// external document too.
func (d *Doc) checkLink(address string, link Link, checkOtherDoc bool) error {
	if address == "" {
		return errors.New("link address not set")
	}

	switch link.Type {
	case externalFile:
		fallthrough
	case externalLink:
		// Check to ensure that referenced file actually exists

		var file string

		if link.ResolvedPath != "" {
			file = link.ResolvedPath
		} else {
			file, _, err := splitLink(address)
			if err != nil {
				return err
			}

			file, err = d.linkAddrToPath(file)
			if err != nil {
				return err
			}

			if !fileExists(file) {
				return d.Errorf("link type %v invalid: %q does not exist",
					link.Type,
					file)
			}
		}

		if link.Type == externalFile {
			break
		}

		// Check the other document
		other, err := getDoc(file, d.Logger)
		if err != nil {
			return err
		}

		if !checkOtherDoc {
			break
		}

		_, section, err := splitLink(address)
		if err != nil {
			return err
		}

		if section == "" {
			break
		}

		if !other.hasHeading(section) {
			return other.Errorf("invalid link %v", address)
		}

	case internalLink:
		// must be a link to an existing heading

		// search for a heading whose LinkName == name
		found := d.headingByLinkName(address)
		if found == nil {
			msg := fmt.Sprintf("failed to find heading for link %q (%+v)", address, link)

			// There is a chance the link description matches the
			// correct heading the link address refers to. In
			// which case, we can derive the correct link address!
			suggestion, err2 := createHeadingID(link.Description)

			if err2 == nil && suggestion != link.Address {
				found = d.headingByLinkName(suggestion)
				if found != nil {
					msg = fmt.Sprintf("%s - correct link name is %q", msg, suggestion)
				}
			}

			return d.Errorf("%s", msg)
		}
	case urlLink:
		// NOP - handled by xurls
	}

	return nil
}

// check performs all checks on the document.
func (d *Doc) check() error {
	for name, linkList := range d.Links {
		for _, link := range linkList {
			err := d.checkLink(name, link, false)
			if err != nil {
				return err
			}
		}
	}

	return nil
}
