// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os"
	"sort"
)

var outputFile = os.Stdout

// displayHandler is an interface that all output display handlers
// (formatters) must implement.
type DisplayHandler interface {
	DisplayLabels(lf *LabelsFile) error
	DisplayCategories(lf *LabelsFile, showLabels bool) error
}

// DisplayHandlers encapsulates the list of available display handlers.
type DisplayHandlers struct {
	handlers map[string]DisplayHandler
}

// handlers is a map of the available output format display handling
// implementations.
var handlers map[string]DisplayHandler

// NewDisplayHandlers create a new DisplayHandler.
func NewDisplayHandlers() *DisplayHandlers {
	if handlers == nil {
		handlers = make(map[string]DisplayHandler)

		handlers["md"] = NewDisplayMD(outputFile)
		handlers[textFormat] = NewDisplayText(outputFile)
		handlers["tsv"] = NewDisplayTSV(outputFile)
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

func show(inputFilename string, handler DisplayHandler, what DataToShow, withLabels bool) error {
	lf, err := readYAML(inputFilename)
	if err != nil {
		return err
	}

	if what == showLabels {
		return handler.DisplayLabels(lf)
	}

	return handler.DisplayCategories(lf, withLabels)
}
