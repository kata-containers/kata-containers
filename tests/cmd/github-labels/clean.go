// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "strings"

func cleanString(s string) string {
	result := strings.Replace(s, "\n", " ", -1)
	result = strings.Replace(result, "\t", "\\t", -1)
	result = strings.TrimSpace(result)

	return result
}

func cleanLabel(l Label) Label {
	return Label{
		Name:         cleanString(l.Name),
		Description:  cleanString(l.Description),
		CategoryName: cleanString(l.CategoryName),
		Colour:       cleanString(l.Colour),
		From:         cleanString(l.From),
	}
}

func cleanCategory(c *Category) {
	c.Name = cleanString(c.Name)
	c.Description = cleanString(c.Description)
	c.URL = cleanString(c.URL)
}

func cleanCategories(lf *LabelsFile) {
	var cleaned Categories

	for _, c := range lf.Categories {
		cleanCategory(&c)
		cleaned = append(cleaned, c)
	}

	lf.Categories = cleaned
}

func cleanLabels(lf *LabelsFile) {
	var cleaned Labels

	for _, l := range lf.Labels {
		new := cleanLabel(l)
		cleaned = append(cleaned, new)
	}

	lf.Labels = cleaned
}

func clean(lf *LabelsFile) {
	lf.Description = cleanString(lf.Description)
	lf.Repo = cleanString(lf.Repo)

	cleanCategories(lf)
	cleanLabels(lf)
}
