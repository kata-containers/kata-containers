// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
)

var storeRoot = "file:///root1/"

func TestNewStore(t *testing.T) {
	s, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)
	assert.Equal(t, s.scheme, "file")
	assert.Equal(t, s.host, "")
	assert.Equal(t, s.path, "/root1/")
}

func TestManagerAddStore(t *testing.T) {
	s, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)
	err = stores.addStore(s)
	defer stores.removeStore(storeRoot)
	assert.Nil(t, err, "addStore failed")

	// Duplicate, should fail
	err = stores.addStore(s)
	assert.NotNil(t, err, "addStore should have failed")

	// Try with an empty URL
	sEmpty, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)
	sEmpty.url = ""
	err = stores.addStore(sEmpty)
	assert.NotNil(t, err, "addStore should have failed on an empty store URL")

}

func TestManagerRemoveStore(t *testing.T) {
	s, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)

	err = stores.addStore(s)
	assert.Nil(t, err, "addStore failed")

	// Positive find
	newStore := stores.findStore(storeRoot)
	assert.NotNil(t, newStore, "findStore failed")

	// Negative removal
	stores.removeStore(storeRoot + "foobar")

	// We should still find storeRoot
	newStore = stores.findStore(storeRoot)
	assert.NotNil(t, newStore, "findStore failed")

	// Positive removal
	stores.removeStore(storeRoot)

	// We should no longer find storeRoot
	newStore = stores.findStore(storeRoot)
	assert.Nil(t, newStore, "findStore should not have found %s", storeRoot)
}

func TestManagerFindStore(t *testing.T) {
	s, err := New(context.Background(), storeRoot)
	assert.Nil(t, err)

	err = stores.addStore(s)
	defer stores.removeStore(storeRoot)
	assert.Nil(t, err, "addStore failed")

	// Positive find
	newStore := stores.findStore(storeRoot)
	assert.NotNil(t, newStore, "findStore failed")

	// Negative find
	newStore = stores.findStore(storeRoot + "foobar")
	assert.Nil(t, newStore, "findStore should not have found a new store")
}
