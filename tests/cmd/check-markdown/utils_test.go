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

func TestSplitLink(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		linkName string
		file     string
		section  string
		valid    bool
	}

	data := []testData{
		{"", "", "", false},

		{"foo.md", "foo.md", "", true},
		{"#bar", "", "bar", true},
		{"foo.md#bar", "foo.md", "bar", true},
		{"foo.md%%bar", "foo.md%%bar", "", true},
	}

	for i, d := range data {
		file, section, err := splitLink(d.linkName)

		if d.valid {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
			assert.Equal(file, d.file, "test[%d]: %+v", i, d)
			assert.Equal(section, d.section, "test[%d]: %+v", i, d)
		} else {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		}
	}
}

func TestValidHeadingIDChar(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		ch    rune
		valid bool
	}

	data := []testData{
		{' ', true},
		{'\t', true},
		{'\n', true},

		{'a', true},
		{'z', true},
		{'A', true},
		{'Z', true},

		{'0', true},
		{'9', true},

		{'-', true},
		{'_', true},

		{'\000', false},
		{'\001', false},
	}

	for i, d := range data {
		result := validHeadingIDChar(d.ch)

		var outcome bool

		if d.valid {
			outcome = result != -1
		} else {
			outcome = result == -1
		}

		assert.Truef(outcome, "test[%d]: %+v", i, d)
	}

	// the main list of invalid chars to test
	invalid := "!@#$%^&*()+=[]{}\\|:\";'<>?,./"

	for i, ch := range invalid {
		result := validHeadingIDChar(ch)

		outcome := result == -1

		assert.Truef(outcome, "invalid[%d]: %+v", i, ch)
	}
}

func TestCreateHeadingID(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		heading      string
		id           string
		expecteError bool
	}

	data := []testData{
		{"", "", true},
		{"a", "a", false},
		{"a.b/c:d", "abcd", false},
		{"a ?", "a-", false},
		{"a !?!", "a-", false},
		{"foo", "foo", false},
		{"foo bar", "foo-bar", false},
		{"foo_bar", "foo_bar", false},
		{"foo_bar()", "foo_bar", false},
		{"`foo_bar()`", "foo_bar", false},
		{"foo_bar()baz", "foo_barbaz", false},
		{"Stability or Performance?", "stability-or-performance", false},
		{"Hello - World", "hello---world", false},
		{"metrics_json_init()", "metrics_json_init", false},
		{"metrics_json_add_array_element(json)", "metrics_json_add_array_elementjson", false},
		{"What is it ?", "what-is-it-", false},
		{"Sandbox `DeviceInfo`", "sandbox-deviceinfo", false},
		{"Build a custom QEMU for aarch64/arm64 - REQUIRED", "build-a-custom-qemu-for-aarch64arm64---required", false},
		{"docker --net=host", "docker---nethost", false},
		{"Containerd Runtime V2 API (Shim V2 API)", "containerd-runtime-v2-api-shim-v2-api", false},
		{"Containerd Runtime V2 API: Shim V2 API", "containerd-runtime-v2-api-shim-v2-api", false},
		{"Launch i3.metal instance", "launch-i3metal-instance", false},
		{"Deploy!", "deploy", false},
	}

	for i, d := range data {
		id, err := createHeadingID(d.heading)

		msg := fmt.Sprintf("test[%d]: %+v, id: %q\n", i, d, id)

		if d.expecteError {
			assert.Error(err)
			continue
		}

		assert.Equal(id, d.id, msg)
	}
}
