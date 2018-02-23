//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/xml"
	"fmt"
	"os"
)

type displayXML struct {
}

// MarshalXML converts a MapSS map type into an XML representation. This is
// required because the XML package is unable to deal with maps itself.
func (m MapSS) MarshalXML(e *xml.Encoder, start xml.StartElement) error {
	tokens := []xml.Token{start}

	for key, value := range m {
		t := xml.StartElement{
			Name: xml.Name{
				Space: "",
				Local: key,
			},
		}

		tokens = append(tokens, t, xml.CharData(value), xml.EndElement{Name: t.Name})
	}

	tokens = append(tokens, xml.EndElement{Name: start.Name})

	for _, t := range tokens {
		err := e.EncodeToken(t)
		if err != nil {
			return err
		}
	}

	return e.Flush()
}

func (d *displayXML) Display(entries *LogEntries, fieldNames []string, file *os.File) error {

	bytes, err := xml.MarshalIndent(entries, displayPrefix, displayIndentValue)
	if err != nil {
		return err
	}

	output := string(bytes)

	_, err = fmt.Fprintln(file, output)

	return err
}
