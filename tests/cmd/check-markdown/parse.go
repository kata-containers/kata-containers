//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"strings"

	bf "gopkg.in/russross/blackfriday.v2"
)

// List of errors found by visitor. Used as the visitor cannot return an error
// directly.
var errorList []error

func (d *Doc) parse() error {
	if !d.ShowTOC && !d.ListMode {
		d.Logger.Info("Checking file")
	}

	err := d.parseMarkdown()
	if err != nil {
		return err
	}

	// mark document as having been handled
	d.Parsed = true

	return nil
}

// parseMarkdown parses the documents markdown.
func (d *Doc) parseMarkdown() error {
	bytes, err := os.ReadFile(d.Name)
	if err != nil {
		return err
	}

	md := bf.New(bf.WithExtensions(bf.CommonExtensions))

	root := md.Parse(bytes)

	root.Walk(makeVisitor(d, d.ShowTOC))

	errorCount := len(errorList)
	if errorCount > 0 {
		extra := ""
		if errorCount != 1 {
			extra = "s"
		}

		var msg []string

		for _, err := range errorList {
			msg = append(msg, err.Error())
		}

		return fmt.Errorf("found %d parse error%s:\n%s",
			errorCount,
			extra,
			strings.Join(msg, "\n"))
	}

	return d.check()
}

// makeVisitor returns a function that is used to visit all document nodes.
//
// If createTOC is false, the visitor will check all nodes, but if true, the
// visitor will only display a table of contents for the document.
func makeVisitor(doc *Doc, createTOC bool) func(node *bf.Node, entering bool) bf.WalkStatus {
	f := func(node *bf.Node, entering bool) bf.WalkStatus {
		if !entering {
			return bf.GoToNext
		}

		var err error

		if createTOC {
			err = doc.displayTOC(node)
		} else {
			err = doc.handleNode(node)
		}

		if err != nil {
			// The visitor cannot return an error, so collect up all parser
			// errors for dealing with later.
			errorList = append(errorList, err)
		}

		return bf.GoToNext
	}

	return f
}
