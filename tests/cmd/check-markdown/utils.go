//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"
	"os"
	"strings"
	"unicode"

	bf "gopkg.in/russross/blackfriday.v2"
)

// fileExists returns true if the specified file exists, else false.
func fileExists(path string) bool {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return false
	}

	return true
}

// splitLink splits a link like "foo.md#section-name" into a filename
// ("foo.md") and a section name ("section-name").
func splitLink(linkName string) (fileName, sectionName string, err error) {
	if linkName == "" {
		return "", "", errors.New("need linkName")
	}

	if !strings.Contains(linkName, anchorPrefix) {
		return linkName, "", nil
	}

	fields := strings.Split(linkName, anchorPrefix)

	expectedFields := 2
	foundFields := len(fields)
	if foundFields != expectedFields {

		return "", "", fmt.Errorf("invalid link %s: expected %d fields, found %d", linkName, expectedFields, foundFields)
	}

	fileName = fields[0]
	sectionName = fields[1]

	return fileName, sectionName, nil
}

// validHeadingIDChar is a strings.Map() function used to determine which characters
// can appear in a heading ID.
func validHeadingIDChar(r rune) rune {
	if unicode.IsLetter(r) ||
		unicode.IsNumber(r) ||
		unicode.IsSpace(r) ||
		r == '-' || r == '_' {
		return r
	}

	// Remove all other chars from destination string
	return -1
}

// createHeadingID creates an HTML anchor name for the specified heading
func createHeadingID(headingName string) (id string, err error) {
	if headingName == "" {
		return "", fmt.Errorf("need heading name")
	}

	// Munge the original heading into an id by:
	//
	// - removing invalid characters.
	// - lower-casing.
	// - replace spaces
	id = strings.Map(validHeadingIDChar, headingName)

	id = strings.ToLower(id)
	id = strings.Replace(id, " ", "-", -1)

	return id, nil
}

func checkNode(node *bf.Node, expectedType bf.NodeType) error {
	if node == nil {
		return errors.New("node cannot be nil")
	}

	if node.Type != expectedType {
		return fmt.Errorf("expected %v node, found %v", expectedType, node.Type)
	}

	return nil
}
