//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"io"
	"io/ioutil"
	"os"
	"path/filepath"
	"regexp"
	"testing"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

type TestReaderNoData struct {
}

func init() {
	name = "log-parser"
}

func (r *TestReaderNoData) Read(p []byte) (n int, err error) {
	return 0, nil
}

type TestReaderData struct {
	called bool
}

func (r *TestReaderData) Read(p []byte) (n int, err error) {
	if r.called {
		// no more data
		return 0, io.EOF
	}

	r.called = true

	data := []byte("level=info pid=1234 source=source name=name msg=msg time=2018-02-24T12:36:36.115882442Z")

	n = copy(p, data)
	return n, nil
}

func TestParseLogFile(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		err = os.RemoveAll(dir)
		assert.NoError(err)
	}()

	type testData struct {
		contents  string
		valid     bool
		ignorable bool
	}

	data := []testData{
		{"", false, false},

		// Unrecognised/invalid fields
		{"foo=", false, false},
		{"foo=bar", false, false},
		{"=bar", false, false},

		// unquoted string value
		{"msg=hello world foo bar", false, false},

		// No level
		{"pid=1234 source=source name=name msg=msg time=2018-02-24T12:36:36.115882442Z", false, true},
		{"level= pid=1234 source=source name=name msg=msg time=2018-02-24T12:36:36.115882442Z", false, true},

		// No pid
		{"level=info source=source name=name msg=msg time=2018-02-24T12:36:36.115882442Z", false, true},
		{"level=info pid= source=source name=name msg=msg time=2018-02-24T12:36:36.115882442Z", false, true},

		// Invalid pid
		{"level=info pid=-1 source=source name=name msg=msg time=2018-02-24T12:36:36.115882442Z", false, false},

		// No source
		{"level=info pid=999 name=name msg=msg time=2018-02-24T12:36:36.115882442Z", false, true},
		{"level=info pid=999 name=name source= msg=msg time=2018-02-24T12:36:36.115882442Z", false, true},

		// No name
		{"level=info pid=1234 source=source msg=msg time=2018-02-24T12:36:36.115882442Z", false, true},
		{"name= level=info pid=1234 source=source msg=msg time=2018-02-24T12:36:36.115882442Z", false, true},

		// Valid
		{"level=info pid=1234 source=source name=name msg=msg time=2018-02-24T12:36:36.115882442Z", true, false},
	}

	for i, d := range data {
		file := filepath.Join(dir, "file.log")
		err := createFile(file, d.contents)
		assert.NoError(err)

		// check that an error is raised when expected
		_, err = parseLogFile(file, false)

		if d.valid {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		}

		// check that the error is ignored when asked to
		_, err = parseLogFile(file, true)
		if d.valid || d.ignorable {
			assert.NoError(err, "test[%d]: %+v", i, d)
		} else {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		}

		err = os.Remove(file)
		assert.NoError(err)
	}
}

func TestParseLogFilesENOENT(t *testing.T) {
	assert := assert.New(t)

	files := []string{"does/not/exist"}

	_, err := parseLogFiles(files, false)
	assert.Error(err)
}

func TestParseLogFiles(t *testing.T) {
	assert := assert.New(t)

	type testFile struct {
		name     string
		contents string
	}

	type testData struct {
		files []testFile
		valid bool
	}

	fooTimeStr := "2018-02-24T12:30:36.115880001Z"
	barTimeStr := "2018-02-24T09:40:40.999999999Z"

	fooData := fmt.Sprintf(`level=info pid=1234 source=foo name=foo-app msg="hello from foo" time=%q`, fooTimeStr)
	barData := fmt.Sprintf(`level=info pid=9876 source=bar name=bar-app msg="hello from bar" time=%q`, barTimeStr)

	data := []testData{
		{
			files: []testFile{
				{"foo.log", ""},
				{"bar.log", ""},
			},

			// empty files are not valid
			valid: false,
		},

		{
			files: []testFile{
				{"foo.log", "=foo"},
				{"bar.log", "=bar"},
			},
			valid: false,
		},

		{
			files: []testFile{
				{"foo.log", "foo="},
				{"bar.log", "bar=baz"},
			},
			valid: false,
		},

		{
			files: []testFile{
				{"foo.log", "time=hello"},
				{"bar.log", "level=source=msg=moo"},
			},
			valid: false,
		},

		{
			files: []testFile{
				{"foo.log", fooData},
				{"bar.log", barData},
			},
			valid: true,
		},
	}

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		err = os.RemoveAll(dir)
		assert.NoError(err)
	}()

	files := []string{"does/not/exist"}

	_, err = parseLogFiles(files, false)
	assert.Error(err)

	for i, d := range data {
		var files []string

		for j, f := range d.files {
			file := filepath.Join(dir, f.name)
			err := createFile(file, f.contents)
			assert.NoErrorf(err, "test[%d] file %d: %+v", i, j, d)
			files = append(files, file)
		}

		e, err := parseLogFiles(files, false)
		if d.valid {
			var fooTime time.Time
			var barTime time.Time

			assert.NoErrorf(err, "test[%d]: %+v", i, d)

			assert.Equal(e.Len(), 2)

			fooTime, err = time.Parse(time.RFC3339Nano, fooTimeStr)
			assert.NoError(err)

			barTime, err = time.Parse(time.RFC3339Nano, barTimeStr)
			assert.NoError(err)

			// check times are now sorted
			assert.Equal(e.Entries[0].Time, barTime)
			assert.Equal(e.Entries[1].Time, fooTime)
		} else {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		}

		// clean up
		for _, f := range d.files {
			file := filepath.Join(dir, f.name)
			err = os.Remove(file)
			assert.NoError(err)
		}
	}
}

func TestParseTime(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		t           time.Time
		timeString  string
		expectError bool
	}

	now := time.Now().UTC()
	nano := now.Format(time.RFC3339Nano)

	time1 := "2018-02-28T03:42:17.310794807Z"
	time2 := "2018-02-28T03:42:17.3107Z"
	time3 := "2018-02-28T03:42:17.00003107Z"
	time4 := "2018-02-28T03:42:17.000031070Z"

	time5 := "2018-02-28T03:42:17.310794807-08:00"
	time6 := "2018-02-28T03:42:17.310794807+07:31"
	time7 := "2018-02-28T03:42:17.31079480+09:44"
	time8 := "2018-02-28T03:42:17.007948-01:01"

	data := []testData{
		{time.Time{}, "", true},

		{now, nano, false},

		{time.Time{}, time1, false},
		{time.Time{}, time2, false},
		{time.Time{}, time3, false},
		{time.Time{}, time4, false},
		{time.Time{}, time5, false},
		{time.Time{}, time6, false},
		{time.Time{}, time7, false},
		{time.Time{}, time8, false},
	}

	for i, d := range data {
		if d.timeString != "" && d.t == (time.Time{}) {
			t, err := time.Parse(time.RFC3339Nano, d.timeString)
			assert.NoError(err)
			d.t = t
		}

		t, err := parseTime(d.timeString)
		if d.expectError {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)

			assert.Equal(d.t, t)
		}
	}
}

func TestCheckKeyValueValid(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		key         string
		value       string
		expectError bool
	}

	data := []testData{
		{"", "", true},
		{"", "value", true},
		{"\x11", "value", true},
		{"key", "\x11", true},
		{"key\x11name", "value", true},
		{"key", "val\x11ue", true},

		{"%!b(MISSING)", "value", true},
		{"%!d(MISSING)", "value", true},
		{"%!v(MISSING)", "value", true},
		{"%!f(MISSING)", "value", true},
		{"%!(BADINDEX)", "value", true},
		{"%!(BADPREC)", "value", true},
		{"%!(BADWIDTH)", "value", true},
		{"%!(EXTRA", "value", true},
		{"%!(EXTRA ", "value", true},

		{"key", "%!b(MISSING)", true},
		{"key", "%!d(MISSING)", true},
		{"key", "%!v(MISSING)", true},
		{"key", "%!f(MISSING)", true},

		{"key", "%!(BADINDEX)", true},
		{"key", "%!(BADPREC)", true},
		{"key", "%!(BADWIDTH)", true},
		{"key", "%!(EXTRA", true},
		{"key", "%!(EXTRA ", true},

		{" ", "value", true},
		{"\n", "value", true},
		{"\t", "value", true},

		// valid
		{"key", " ", false},
		{"key", "\t", false},
		{"key", "\n", false},
		{"key", "value", false},
	}

	for i, d := range data {
		err := checkKeyValueValid(d.key, d.value)

		if d.expectError {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
		}
	}
}

func TestHandleLogEntry(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		le          *LogEntry
		key         string
		value       string
		expectError bool
	}

	data := []testData{
		{nil, "", "", true},
		{&LogEntry{}, "", "", true},
		{&LogEntry{}, "pid", "hello", true},
		{&LogEntry{}, "time", "not a time", true},
		{&LogEntry{
			Data: map[string]string{
				"hello": "world",
			},
		}, "hello", "world", true},

		// Valid
		{&LogEntry{}, "key", "value", false},
	}

	for i, d := range data {
		if d.le != nil && d.le.Data == nil {
			d.le.Data = make(map[string]string)
		}

		err := handleLogEntry(d.le, d.key, d.value)

		if d.expectError {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
		}
	}
}

func TestCreateLogEntry(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		file        string
		pairs       kvPairs
		expectError bool
		line        uint64
	}

	kernelMsg := regexp.QuoteMeta(`[    1.122452] sd 0:0:0:0: [sda] 20971520 512-byte logical blocks: (10.7 GB/10.0 GiB)`)

	now := time.Now().UTC()
	nano := now.Format(time.RFC3339Nano)

	// simulate kernel write to console which will "corrupt" the
	// agent log output.
	corruptTestDataPairs := kvPairs{
		{"time", nano},
		{"source", "agent"},
		{"msg", fmt.Sprintf("time=%s%s", nano, kernelMsg)},
	}

	originalStrict := strict
	originalLogLevel := logger.Logger.Level

	defer func() {
		strict = originalStrict
		logger.Logger.Level = originalLogLevel
	}()

	// enable rigorous checking
	strict = true

	// hide warnings
	logger.Logger.SetLevel(logrus.ErrorLevel)

	strictData := []testData{
		{"", kvPairs{}, true, 0},
		{"foo", kvPairs{}, true, 0},
		{"", kvPairs{}, true, 1},
		{"foo", kvPairs{}, true, 1},
		{"foo", kvPairs{{"key", "\x11"}}, true, 1},
		{"foo", kvPairs{{"\x00", "value"}}, true, 1},
		{"foo", kvPairs{{" ", "value"}}, true, 1},
		{"foo", kvPairs{{"\t", "value"}}, true, 1},
		{"foo", kvPairs{{"\n", "value"}}, true, 1},
		{"foo", kvPairs{{"key", "value"}}, true, 0},
		{"", kvPairs{{"key", "value"}}, true, 1},
		{"/some/where", corruptTestDataPairs, true, 1},

		// valid
		{"foo", kvPairs{{"key", "value"}}, false, 1},

		{"foo", kvPairs{{"key", ""}}, false, 1},
		{"foo", kvPairs{{"key", " "}}, false, 1},
		{"foo", kvPairs{{"key", "\t"}}, false, 1},
		{"foo", kvPairs{{"key", "\n"}}, false, 1},
		{"foo", kvPairs{{"key", `\t`}}, false, 1},
		{"foo", kvPairs{{"key", `\n`}}, false, 1},
		{"foo", kvPairs{{"key", "foo bar"}}, false, 1},
	}

	for i, d := range strictData {
		_, err := createLogEntry(d.file, d.line, d.pairs)
		if d.expectError {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
		}
	}

	// disable rigorous checking
	strict = false

	nonStrictData := []testData{
		{"/some/where", corruptTestDataPairs, false, 1},
	}

	for i, d := range nonStrictData {
		_, err := createLogEntry(d.file, d.line, d.pairs)
		if d.expectError {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
		}
	}
}

func TestCreateLogEntryAgentUnpack(t *testing.T) {
	assert := assert.New(t)

	file := "/foo/bar.log"
	line := uint64(1)
	level := "debug"
	source := "agent"
	version := "0.0.1-71de96fb62a7e13f9d336c86564984a5188a9d7a"

	now := time.Now().UTC()
	timestamp := now.Format(time.RFC3339Nano)

	name := "foo"
	value := "hello world"

	agentFields := fmt.Sprintf("time=%s"+
		" name=%s"+
		" source=agent"+
		" level=%s"+
		" pid=%d"+
		" version=%s"+
		" %s=%q",
		timestamp,
		testName,
		level,
		testPid,
		version,
		name,
		value)

	expectedLogEntry := LogEntry{
		Count:    0,
		Filename: file,
		Line:     line,
		Pid:      testPid,
		Level:    level,
		Source:   source,
		Name:     testName,
		Time:     now,
		Data: map[string]string{
			"version": version,
			name:      value,
		},
	}

	agentPairs := kvPairs{
		{"source", "agent"},
		{"level", "info"},
		{"msg", agentFields},
	}

	disableAgentUnpack = false
	agent, err := createLogEntry(file, line, agentPairs)
	assert.NoError(err)
	assert.Equal(agent, expectedLogEntry)
}

func TestParseLogFmtDataNoReaderData(t *testing.T) {
	assert := assert.New(t)

	file := "/foo/bar.log"

	reader := &TestReaderNoData{}

	entries, err := parseLogFmtData(reader, file, false)

	// reader returns no data, which is invalid
	assert.Error(err)
	assert.Equal(entries.Len(), 0)

}

func TestParseLogFmtData(t *testing.T) {
	assert := assert.New(t)

	file := "/foo/bar.log"

	reader := &TestReaderData{}

	entries, err := parseLogFmtData(reader, file, false)
	assert.NoError(err)
	assert.Equal(entries.Len(), 1)
}
