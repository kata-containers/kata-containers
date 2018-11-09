// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"fmt"
	"io/ioutil"
	"regexp"
	"strings"
	"testing"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

type testData struct {
	network     string
	raddr       string
	expectError bool
}

func init() {
	// Ensure all log levels are logged
	kataUtilsLogger.Logger.Level = logrus.DebugLevel

	// Discard log output
	kataUtilsLogger.Logger.Out = ioutil.Discard
}

func TestHandleSystemLog(t *testing.T) {
	assert := assert.New(t)

	data := []testData{
		{"invalid-net-type", "999.999.999.999", true},
		{"invalid net-type", "a a ", true},
		{"invalid-net-type", ".", true},
		{"moo", "999.999.999.999", true},
		{"moo", "999.999.999.999:99999999999999999", true},
		{"qwerty", "uiop:ftw!", true},
		{"", "", false},
	}

	for _, d := range data {
		err := handleSystemLog(d.network, d.raddr)
		if d.expectError {
			assert.Error(err, fmt.Sprintf("%+v", d))
		} else {
			assert.NoError(err, fmt.Sprintf("%+v", d))
		}
	}
}

func TestNewSystemLogHook(t *testing.T) {
	assert := assert.New(t)

	hook, err := newSystemLogHook("", "")
	assert.NoError(err)

	msg := "wibble"
	level := logrus.DebugLevel

	logger := logrus.New()

	// ensure all output is displayed
	logger.Level = logrus.DebugLevel

	// throw away all stdout so that the Format() call
	// below returns the data in structured form.
	logger.Out = ioutil.Discard

	entry := &logrus.Entry{
		Logger: logger,

		// UTC for time.Parse()
		Time: time.Now().UTC(),

		Message: msg,
		Level:   level,
	}

	// call the formatting function directly and see if the output
	// matches what we expect.
	bytes, err := hook.formatter.Format(entry)
	assert.NoError(err)

	output := string(bytes)
	output = strings.TrimSpace(output)
	output = strings.Replace(output, `"`, "", -1)

	fields := strings.Fields(output)

	msgFound := ""
	timeFound := ""
	levelFound := ""

	// look for the expected fields
	for _, field := range fields {

		// split each structured field into name and value fields
		f := strings.Split(field, "=")
		assert.True(len(f) >= 2)

		switch f[0] {
		case "level":
			levelFound = f[1]
		case "msg":
			msgFound = f[1]
		case "time":
			timeFound = f[1]
		}
	}

	assert.Equal(levelFound, level.String())
	assert.Equal(msgFound, msg)
	assert.NotEqual(timeFound, "")

	// Tell time.Parse() how to handle the timestamps by putting it into
	// the standard golang time format equivalent to:
	//
	//     "Mon Jan 2 15:04:05 -0700 MST 2006".
	//
	expectedTimeFormat := "2006-01-02T15:04:05.999999999Z"

	// Note that time.Parse() assumes a UTC time.
	_, err = time.Parse(expectedTimeFormat, timeFound)
	assert.NoError(err)

	// time.Parse() is "clever" but also doesn't check anything more
	// granular than a second, so let's be completely paranoid and check
	// via regular expression too.
	expectedPattern :=
		// YYYY-MM-DD
		`\d{4}-\d{2}-\d{2}` +
			// time separator
			`T` +
			// HH:MM:SS
			`\d{2}:\d{2}:\d{2}` +
			// high-precision separator
			`.` +
			// nano-seconds. Note that the quantifier range is
			// required because the time.RFC3339Nano format
			// trunctates trailing zeros.
			`\d{1,9}` +
			// UTC timezone specifier
			`Z`

	expectedRE := regexp.MustCompile(expectedPattern)
	matched := expectedRE.FindAllStringSubmatch(timeFound, -1)
	assert.NotNil(matched, "expected time in format %q, got %q", expectedPattern, timeFound)
}
