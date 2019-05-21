//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

// newHeading creates a new Heading.
func newHeading(name, mdName, linkName string, level int) Heading {
	return Heading{
		Name:     name,
		MDName:   mdName,
		LinkName: linkName,
		Level:    level,
	}
}
