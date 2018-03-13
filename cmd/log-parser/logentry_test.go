//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"sort"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestNewTimeDelta(t *testing.T) {
	assert := assert.New(t)

	duration := time.Nanosecond
	d := NewTimeDelta(duration)
	assert.Equal(d, TimeDelta(duration))
}

func TestNewTimeDeltaString(t *testing.T) {
	assert := assert.New(t)

	duration := time.Second * 65
	d := NewTimeDelta(duration)

	nano := duration * time.Nanosecond

	expected := fmt.Sprintf("%d", nano)

	assert.Equal(d.String(), expected)
}

func TestLogEntryCheck(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		le    LogEntry
		valid bool
	}

	data := []testData{
		{LogEntry{}, false},

		{
			// No Filename
			LogEntry{
				Line:   1,
				Time:   time.Now().UTC(),
				Pid:    123,
				Level:  "debug",
				Source: "source",
				Name:   "name",
			},
			false,
		},

		{
			// No Line
			LogEntry{
				Filename: "/foo/bar",
				Time:     time.Now().UTC(),
				Pid:      123,
				Level:    "debug",
				Source:   "source",
				Name:     "name",
			},
			false,
		},

		{
			// No Time
			LogEntry{
				Filename: "/foo/bar",
				Line:     1,
				Pid:      123,
				Level:    "debug",
				Source:   "source",
				Name:     "name",
			},
			false,
		},

		{
			// No Pid
			LogEntry{
				Filename: "/foo/bar",
				Line:     1,
				Time:     time.Now().UTC(),
				Level:    "debug",
				Source:   "source",
				Name:     "name",
			},
			false,
		},

		{
			// No Level
			LogEntry{
				Filename: "/foo/bar",
				Line:     1,
				Time:     time.Now().UTC(),
				Pid:      123,
				Source:   "source",
				Name:     "name",
			},
			false,
		},

		{
			// No Source
			LogEntry{
				Filename: "/foo/bar",
				Line:     1,
				Time:     time.Now().UTC(),
				Pid:      123,
				Level:    "debug",
				Name:     "name",
			},
			false,
		},

		{
			// No Name
			LogEntry{
				Filename: "/foo/bar",
				Line:     1,
				Time:     time.Now().UTC(),
				Pid:      123,
				Level:    "debug",
				Source:   "source",
			},
			false,
		},

		{
			LogEntry{
				Filename: "/foo/bar",
				Line:     1,
				Time:     time.Now().UTC(),
				Pid:      123,
				Level:    "debug",
				Source:   "source",
				Name:     "name",
			},
			true,
		},

		{
			LogEntry{
				Filename: "-",
				Line:     1,
				Time:     time.Now().UTC(),
				Pid:      123,
				Level:    "debug",
				Source:   "source",
				Name:     "name",
			},
			true,
		},
	}

	for i, d := range data {
		err := d.le.Check()

		if d.valid {
			assert.NoErrorf(err, "test[%d]: %+v", i, d)
		} else {
			assert.Errorf(err, "test[%d]: %+v", i, d)
		}
	}
}

func TestLogEntriesLen(t *testing.T) {
	assert := assert.New(t)

	e := LogEntries{}
	assert.Equal(e.Len(), 0)

	e = LogEntries{
		Entries: []LogEntry{
			{},
			{},
			{},
		},
	}
	assert.Equal(e.Len(), 3)
}

func TestLogEntriesSwap(t *testing.T) {
	assert := assert.New(t)

	e := LogEntries{
		Entries: []LogEntry{
			{Name: "first"},
			{Name: "second"},
		},
	}

	assert.Equal(e.Entries[0].Name, "first")
	assert.Equal(e.Entries[1].Name, "second")

	e.Swap(1, 0)

	assert.Equal(e.Entries[0].Name, "second")
	assert.Equal(e.Entries[1].Name, "first")

	e.Swap(0, 1)

	assert.Equal(e.Entries[0].Name, "first")
	assert.Equal(e.Entries[1].Name, "second")
}

func TestLogEntriesLess(t *testing.T) {
	assert := assert.New(t)

	now := time.Now().UTC()
	later := now.Add(time.Second * 1)

	e := LogEntries{
		Entries: []LogEntry{
			{Time: now},
			{Time: later},
		},
	}

	assert.True(e.Less(0, 1))
	e.Swap(0, 1)
	assert.True(e.Less(1, 0))
}

func TestLogEntrySort(t *testing.T) {
	assert := assert.New(t)

	now := time.Now().UTC()
	later := now.Add(time.Second * 7)
	latest := later.Add(time.Second * 13)

	entries := []LogEntry{
		{Time: later},
		{Time: latest},
		{Time: now},
	}

	le := LogEntries{
		Entries: entries,
	}

	sort.Sort(le)

	assert.Equal(le.Entries[0].Time, now)
	assert.Equal(le.Entries[1].Time, later)
	assert.Equal(le.Entries[2].Time, latest)
}
