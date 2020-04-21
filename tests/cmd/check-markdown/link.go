//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"os"
	"path/filepath"
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

		file, _, err := splitLink(address)
		if err != nil {
			return err
		}

		file, err = l.Doc.linkAddrToPath(file)
		if err != nil {
			return err
		}

		l.ResolvedPath = file
	} else {
		isREADME, err := l.handleImplicitREADME()
		if err != nil {
			return err
		}

		if !isREADME {
			// Link must be an external file, but not a markdown file.
			l.Type = externalFile
		}
	}

	return nil
}

// handleImplicitREADME determines if the specified link is an implicit link
// to a README document.
func (l *Link) handleImplicitREADME() (isREADME bool, err error) {
	const readme = "README.md"

	address := l.Address
	if address == "" {
		return false, errors.New("need link address")
	}

	file, err := l.Doc.linkAddrToPath(address)
	if err != nil {
		return false, err
	}

	// The resolved path should exist as this is a local file.
	st, err := os.Stat(file)
	if err != nil {
		return false, err
	}

	if !st.IsDir() {
		return false, nil
	}

	// The file is a directory so try appending the implicit README file
	// and see if that exists.
	resolvedPath := filepath.Join(file, readme)

	success := fileExists(resolvedPath)

	if success {
		l.Type = externalLink
		l.ResolvedPath = resolvedPath
	}

	return success, nil
}
