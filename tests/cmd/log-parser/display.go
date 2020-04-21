//
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"io"
	"os"
	"sort"
	"strings"
	"text/template"
)

// headerTemplate is used by addCommentHeader().
const headerTemplate = `
#----------------------------------------
# Name:           {{.name}}
# Version:        {{.version}}
# Commit:         {{.commit}}
# Fields:         {{.fields}}
# Format Version: {{.formatVersion}}
#----------------------------------------

`

var (
	// displayPrefix specifies a display prefix value
	displayPrefix = ""

	// displayIndentValue specifies a space-indent value
	displayIndentValue = strings.Repeat(" ", 4)
)

// displayHandler is an interface that all output display handlers
// (formatters) must implement.
type displayHandler interface {
	// Display must write the log entries to the specified file. If the
	// format supports it, fieldNames can be added to the output.
	Display(entries *LogEntries, fieldNames []string, file *os.File) error
}

// DisplayHandlers encapsulates the list of available display handlers.
type DisplayHandlers struct {
	handlers map[string]displayHandler
}

// handlers is a map of the available output format display handling
// implementations.
var handlers = map[string]displayHandler{
	"csv":  &displayCSV{},
	"json": &displayJSON{},
	"text": &displayText{},
	"toml": &displayTOML{},
	"xml":  &displayXML{},
	"yaml": &displayYAML{},
}

// NewDisplayHandlers create a new displayHandler.
func NewDisplayHandlers() *DisplayHandlers {
	h := &DisplayHandlers{
		handlers: handlers,
	}

	return h
}

// find looks for a display handler corresponding to the specified format
func (d *DisplayHandlers) find(format string) displayHandler {
	for f, handler := range d.handlers {
		if f == format {
			return handler
		}
	}

	return nil
}

// supplementEntries sets extra fields in the list of log entries
func (d *DisplayHandlers) supplementEntries(entries *LogEntries) {
	records := uint64(len(entries.Entries))
	var i uint64

	for i = 0; i < records; i++ {
		this := &entries.Entries[i]
		this.Count = 1 + i

		if i > 0 {
			// only calculate time difference for 2nd and
			// subsequent records as the first record doesn't have
			// a record before it :)
			prev := &entries.Entries[i-1]

			this.TimeDelta = NewTimeDelta(this.Time.Sub(prev.Time))
		}
	}
}

// Handle adds the record count and timedeltas to the records and then calls
// the display handler.
//
// Note: The LogEntries are assumed to have already been sorted by
// LogEntry.Time.
func (d *DisplayHandlers) Handle(entries *LogEntries, format string, file *os.File) (err error) {
	handler := d.find(format)
	if handler == nil {
		return fmt.Errorf("no display handler for %v", format)
	}

	d.supplementEntries(entries)

	le := LogEntry{}
	fieldNames := le.Fields()

	return handler.Display(entries, fieldNames, file)
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

// addCommentHeader can be used to add a header containing some metadata
// for those format that support "#" comments.
func addCommentHeader(fieldNames []string, writer io.Writer) error {
	t := template.New("")

	t, err := t.Parse(headerTemplate)
	if err != nil {
		return err
	}

	args := map[string]string{
		"name":          name,
		"version":       version,
		"commit":        commit,
		"fields":        strings.Join(fieldNames, ","),
		"formatVersion": logEntryFormatVersion,
	}

	return t.Execute(writer, args)
}
