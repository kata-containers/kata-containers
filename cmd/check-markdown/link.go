//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"regexp"
	"strings"
)

// newLink creates a new Link.
func newLink(doc *Doc, address, description string) (Link, error) {
	l := Link{
		Doc:         doc,
		Address:     address,
		Description: description,
	}

	err := l.categorise()
	if err != nil {
		return Link{}, err
	}

	return l, nil
}

// categorise determines the type of Link.
func (l *Link) categorise() error {
	address := l.Address

	// markdown file extension with optional link name ("#...")
	const re = `\.md#*.*$`

	pattern := regexp.MustCompile(re)

	matched := pattern.MatchString(address)

	if strings.HasPrefix(address, "http:") {
		l.Type = urlLink
	} else if strings.HasPrefix(address, "https:") {
		l.Type = urlLink
	} else if strings.HasPrefix(address, "mailto:") {
		l.Type = mailLink
	} else if strings.HasPrefix(address, anchorPrefix) {
		l.Type = internalLink

		// Remove the prefix to make a valid link address
		address = strings.TrimPrefix(address, anchorPrefix)
		l.Address = address
	} else if matched {
		l.Type = externalLink
	} else {
		// Link must be an external file, but not a markdown file.
		l.Type = externalFile
	}

	return nil
}
