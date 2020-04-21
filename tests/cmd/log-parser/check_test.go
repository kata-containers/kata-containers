//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestCheckValid(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		value string
		valid bool
	}

	data := []testData{
		{"", true},
		{" ", true},
		{"\t", true},
		{"\n", true},
		{`\t`, true},
		{`\n`, true},
		{"\x00", false},
		{"\x11", false},
		{"hello\x00", false},
		{"hello\x00world", false},
		{"\x00hello", false},
		{"world\x00", false},
		{`\x00`, true},
		{`\x11`, true},

		{"%!d(MISSING)", false},
		{"%!f(MISSING)", false},
		{"%!v(MISSING)", false},

		{"%!(BADINDEX)", false},
		{"%!(BADPREC)", false},
		{"%!(BADWIDTH)", false},
		{"%!(EXTRA ", false},
		{"%!(EXTRA ", false},
		{"%!(EXTRA string=hello)", false},
	}

	for i, d := range data {
		err := checkValid(d.value)

		if d.valid {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		}
	}
}
