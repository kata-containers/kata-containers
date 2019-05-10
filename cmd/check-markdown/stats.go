//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"

	"github.com/Sirupsen/logrus"
)

func (d *Doc) showStats() {
	var counters [LinkTypeCount]int

	linkCount := 0

	for _, link := range d.Links {
		counters[link.Type]++
		linkCount++
	}

	fields := logrus.Fields{
		"headings-count": len(d.Headings),
		"links-count":    linkCount,
	}

	for i, count := range counters {
		name := LinkType(i).String()

		fieldName := fmt.Sprintf("link-type-%s-count", name)

		fields[fieldName] = count
	}

	d.Logger.WithFields(fields).Info("Statistics")
}
