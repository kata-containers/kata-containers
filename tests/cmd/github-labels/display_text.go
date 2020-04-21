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

func (d *displayText) DisplayLabels(lf *LabelsFile) error {
	_, err := fmt.Fprintf(d.file, "Labels (count: %d):\n", len(lf.Labels))
	if err != nil {
		return err
	}

	for _, l := range lf.Labels {
		err = d.displayLabel(l)
		if err != nil {
			return err
		}
	}

	return nil
}

func (d *displayText) displayLabel(l Label) error {
	_, err := fmt.Fprintf(d.file, "    %s (%q) [category %q, colour %q, from %q]\n",
		l.Name,
		l.Description,
		l.CategoryName,
		l.Colour,
		l.From)

	return err
}

func (d *displayText) DisplayCategories(lf *LabelsFile, showLabels bool) error {
	_, err := fmt.Fprintf(d.file, "Categories (count: %d):\n", len(lf.Categories))
	if err != nil {
		return err
	}

	for _, c := range lf.Categories {
		err := d.displayCategory(c, lf, showLabels)
		if err != nil {
			return err
		}
	}

	return nil
}

func (d *displayText) displayCategory(c Category, lf *LabelsFile, showLabels bool) error {
	if showLabels {
		labels, err := getLabelsByCategory(c.Name, lf)
		if err != nil {
			return err
		}

		_, err = fmt.Fprintf(d.file, "    %s (%q, label count: %d, url: %v)\n",
			c.Name,
			c.Description,
			len(labels),
			c.URL)
		if err != nil {
			return err
		}

		for _, label := range labels {
			_, err := fmt.Fprintf(d.file, "        %s (%q)\n",
				label.Name,
				label.Description)
			if err != nil {
				return err
			}
		}
	} else {
		_, err := fmt.Printf("    %s (%q, url: %v)\n",
			c.Name,
			c.Description,
			c.URL)
		if err != nil {
			return err
		}
	}

	return nil
}
