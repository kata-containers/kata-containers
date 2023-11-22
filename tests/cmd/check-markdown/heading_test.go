//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewHeading(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		headingName      string
		mdName           string
		expectedLinkName string
		level            int
		expectError      bool
	}

	data := []testData{
		{"", "", "", -1, true},
		{"a", "", "", -1, true},
		{"a", "a", "", -1, true},
		{"a", "a", "", 0, true},
		{"a", "", "", 1, true},

		{"a", "a", "a", 1, false},
		{"a-b", "`a-b`", "`a-b`", 1, false},
		{"a_b", "`a_b`", "`a_b`", 1, false},
		{"foo (json) bar", "foo `(json)` bar", "foo-json-bar", 1, false},
		{"func(json)", "`func(json)`", "funcjson", 1, false},
		{"?", "?", "", 1, false},
		{"a b", "a b", "a-b", 1, false},
		{"a - b", "a - b", "a---b", 1, false},
		{"a - b?", "a - b?", "a---b", 1, false},
		{"a - b.", "a - b.", "a---b", 1, false},
		{"a:b", "a:b", "ab", 1, false},
		{"a;b", "a;b", "ab", 1, false},
		{"a@b", "a@b", "ab", 1, false},
		{"a+b", "a+b", "ab", 1, false},
		{"a,b", "a,b", "ab", 1, false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v\n", i, d)

		h, err := newHeading(d.headingName, d.mdName, d.level)
		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.Equal(h.Name, d.headingName, msg)
		assert.Equal(h.MDName, d.mdName, msg)
		assert.Equal(h.Level, d.level, msg)
		assert.Equal(h.LinkName, d.expectedLinkName, msg)
	}
}
