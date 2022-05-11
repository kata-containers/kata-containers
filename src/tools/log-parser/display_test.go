//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestNewDisplayHandlers(t *testing.T) {
	assert := assert.New(t)

	h := NewDisplayHandlers()
	assert.NotEmpty(h.handlers)
}

type mockDisplayHandler struct {
}

var handlerCalled = false

func (m *mockDisplayHandler) Display(entries *LogEntries, fieldNames []string, file *os.File) error {
	handlerCalled = true

	return nil
}

func TestDisplayHandlersFind(t *testing.T) {
	assert := assert.New(t)

	origHandlers := handlers
	handlers = map[string]displayHandler{}

	defer func() {
		handlers = origHandlers
	}()

	h := NewDisplayHandlers()
	assert.Empty(h.handlers)

	assert.Nil(h.find("foo"))

	handlers = map[string]displayHandler{
		"foo": &mockDisplayHandler{},
	}

	h = NewDisplayHandlers()
	assert.NotEmpty(h.handlers)

	assert.NotNil(h.find("foo"))
	assert.Equal(h.find("foo"), &mockDisplayHandler{})
}

func TestDisplayHandlersGet(t *testing.T) {
	assert := assert.New(t)

	origHandlers := handlers
	handlers = map[string]displayHandler{
		"foo": &mockDisplayHandler{},
		"bar": &mockDisplayHandler{},
		"baz": &mockDisplayHandler{},
	}

	defer func() {
		handlers = origHandlers
	}()

	h := NewDisplayHandlers()
	assert.NotEmpty(h.handlers)

	// list should be sorted
	expected := []string{"bar", "baz", "foo"}
	assert.Equal(expected, h.Get())
}

func TestDisplayHandlersSupplementEntries(t *testing.T) {
	assert := assert.New(t)

	now := time.Now().UTC()
	later := now.Add(time.Second * 1)
	diff := later.Sub(now)

	entries := []LogEntry{
		{Time: now},
		{Time: later},
	}

	le := LogEntries{
		Entries: entries,
	}

	d := NewDisplayHandlers()
	assert.NotEmpty(d.handlers)

	d.supplementEntries(&le)

	assert.Equal(entries[0].Count, uint64(1))
	assert.Equal(entries[0].Time, now)
	assert.Equal(entries[0].TimeDelta, NewTimeDelta(0))

	assert.Equal(entries[1].Count, uint64(2))
	assert.Equal(entries[1].Time, later)
	assert.Equal(entries[1].TimeDelta, NewTimeDelta(diff))

}

func TestDisplayHandlersHandle(t *testing.T) {
	assert := assert.New(t)

	now := time.Now().UTC()
	later := now.Add(time.Second * 1)

	entries := []LogEntry{
		{Time: now},
		{Time: later},
	}

	le := LogEntries{
		Entries: entries,
	}

	origHandlers := handlers
	handlers = map[string]displayHandler{
		"foo": &mockDisplayHandler{},
	}

	defer func() {
		handlers = origHandlers
	}()

	d := NewDisplayHandlers()
	assert.NotEmpty(d.handlers)

	assert.False(handlerCalled)

	err := d.Handle(&le, "invalid", os.Stdout)
	assert.Error(err)

	err = d.Handle(&le, "foo", os.Stdout)
	assert.NoError(err)
	assert.True(handlerCalled)
}
