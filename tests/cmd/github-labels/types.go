// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

type Category struct {
	Name        string
	Description string
	URL         string `yaml:",omitempty"`
}

type Label struct {
	Name         string
	Description  string
	CategoryName string `yaml:"category"`
	Colour       string `yaml:"color"`
	From         string `yaml:",omitempty"`
}

type Categories []Category

func (c Categories) Len() int {
	return len(c)
}

func (c Categories) Swap(i, j int) {
	c[i], c[j] = c[j], c[i]
}

func (c Categories) Less(i, j int) bool {
	return c[i].Name < c[j].Name
}

type Labels []Label

func (l Labels) Len() int {
	return len(l)
}

func (l Labels) Swap(i, j int) {
	l[i], l[j] = l[j], l[i]
}

func (l Labels) Less(i, j int) bool {
	return l[i].Name < l[j].Name
}

type LabelsFile struct {
	Description string
	Categories  Categories
	Repo        string
	Labels      Labels
}
