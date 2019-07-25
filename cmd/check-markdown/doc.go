//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"

	"github.com/sirupsen/logrus"
)

// Details of the main document, and all other documents it references.
// Key: document name.
var docs map[string]*Doc

func init() {
	docs = make(map[string]*Doc)
}

// newDoc creates a new document.
func newDoc(name string, logger *logrus.Entry) *Doc {
	d := &Doc{
		Name:     name,
		Headings: make(map[string]Heading),
		Links:    make(map[string][]Link),
		Parsed:   false,
		ShowTOC:  false,
		Logger:   logger,
	}

	d.Logger = logger.WithField("file", d.Name)

	// add to the hash
	docs[name] = d

	return d
}

// getDoc returns the Doc structure represented by the specified name,
// creating it and adding to the docs map if necessary.
func getDoc(name string, logger *logrus.Entry) (*Doc, error) {
	if name == "" {
		return &Doc{}, errors.New("need doc name")
	}

	doc, ok := docs[name]
	if ok {
		return doc, nil
	}

	return newDoc(name, logger), nil
}

// hasHeading returns true if the specified heading exists for the document.
func (d *Doc) hasHeading(name string) bool {
	return d.heading(name) != nil
}

// Errorf is a convenience function to generate an error for this particular
// document.
func (d *Doc) Errorf(format string, args ...interface{}) error {
	s := fmt.Sprintf(format, args...)

	return fmt.Errorf("file=%q: %s", d.Name, s)
}

// String "pretty-prints" the specified document
//
// Just display the name as that is enough in text output.
func (d *Doc) String() string {
	return d.Name
}
