//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"sort"

	"github.com/sirupsen/logrus"
)

var outputFile = os.Stdout

// displayHandler is an interface that all output display handlers
// (formatters) must implement.
type DisplayHandler interface {
	DisplayHeadings(d *Doc) error
	DisplayLinks(d *Doc) error
}

// DisplayHandlers encapsulates the list of available display handlers.
type DisplayHandlers struct {
	handlers map[string]DisplayHandler
}

// handlers is a map of the available output format display handling
// implementations.
var handlers map[string]DisplayHandler

// NewDisplayHandlers create a new DisplayHandler.
func NewDisplayHandlers(tsvSeparator string, disableHeader bool) *DisplayHandlers {
	separator := rune('\t')

	if tsvSeparator != "" {
		separator = rune(tsvSeparator[0])
	}

	if handlers == nil {
		handlers = make(map[string]DisplayHandler)

		handlers[textFormat] = NewDisplayText(outputFile)
		handlers[tsvFormat] = NewDisplayTSV(outputFile, separator, disableHeader)
	}

	h := &DisplayHandlers{
		handlers: handlers,
	}

	return h
}

// find looks for a display handler corresponding to the specified format
func (d *DisplayHandlers) find(format string) DisplayHandler {
	for f, handler := range d.handlers {
		if f == format {
			return handler
		}
	}

	return nil
}

// Get returns a list of the available formatters (display handler names).
func (d *DisplayHandlers) Get() []string {
	var formats []string

	for f := range d.handlers {
		formats = append(formats, f)
	}

	sort.Strings(formats)

	return formats
}

func show(inputFilename string, logger *logrus.Entry, handler DisplayHandler, what DataToShow) error {
	var fn func(*Doc) error

	switch what {
	case showHeadings:
		fn = handler.DisplayHeadings
	case showLinks:
		fn = handler.DisplayLinks
	default:
		return fmt.Errorf("unknown show option: %v", what)
	}

	doc := newDoc(inputFilename, logger)
	doc.ListMode = true

	err := doc.parse()
	if err != nil {
		return err
	}

	return fn(doc)
}
