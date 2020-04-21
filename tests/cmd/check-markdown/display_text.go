// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
)

type displayText struct {
	file *os.File
}

func NewDisplayText(file *os.File) DisplayHandler {
	return &displayText{
		file: file,
	}
}

func (d *displayText) DisplayLinks(doc *Doc) error {
	for _, linkList := range doc.Links {
		for _, link := range linkList {
			err := d.displayLink(link)
			if err != nil {
				return err
			}
		}
	}

	return nil
}

func (d *displayText) displayLink(l Link) error {
	_, err := fmt.Fprintf(d.file, "%+v\n", l)

	return err
}

func (d *displayText) DisplayHeadings(doc *Doc) error {
	for _, h := range doc.Headings {
		err := d.displayHeading(h)
		if err != nil {
			return err
		}
	}

	return nil
}

func (d *displayText) displayHeading(h Heading) error {
	_, err := fmt.Fprintf(d.file, "%+v\n", h)

	return err
}
