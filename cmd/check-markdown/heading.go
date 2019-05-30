//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "fmt"

// newHeading creates a new Heading.
func newHeading(name, mdName string, level int) (Heading, error) {
	if name == "" {
		return Heading{}, fmt.Errorf("heading name cannot be blank")
	}

	if mdName == "" {
		return Heading{}, fmt.Errorf("heading markdown name cannot be blank")
	}

	linkName, err := createHeadingID(name)
	if err != nil {
		return Heading{}, err
	}

	if level < 1 {
		return Heading{}, fmt.Errorf("level needs to be atleast 1")
	}

	return Heading{
		Name:     name,
		MDName:   mdName,
		LinkName: linkName,
		Level:    level,
	}, nil
}
