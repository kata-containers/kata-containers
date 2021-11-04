// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package uuid

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

// Test UUID parsing and string conversation.
//
// This test simply converts a set of strings to UUIDs and back again.
//
// The original strings and the strings generated from the UUIDs match.
func TestUUID(t *testing.T) {
	assert := assert.New(t)
	testUUIDs := []string{
		"f81d4fae-7dec-11d0-a765-00a0c91e6bf6",
		"30dedd5c-48d9-45d3-8b44-f973e4f35e48",
		"69e84267-ed01-4738-b15f-b47de06b62e7",
		"e35ed972-c46c-4aad-a1e7-ef103ae079a2",
		"eba04826-62a5-48bd-876f-9119667b1487",
		"ca957444-fa46-11e5-94f9-38607786d9ec",
		"ab68111c-03a6-11e6-87de-001320fb6e31",
	}

	for _, s := range testUUIDs {
		uuid, err := Parse(s)
		assert.NoError(err)
		s2 := uuid.String()
		assert.Equal(s, s2)
	}
}

// Test UUID generation.
//
// This test generates 100 new UUIDs and then verifies that those UUIDs
// can be parsed.
//
// The UUIDs are generated correctly, their version number is correct,
// and they can be parsed.
func TestGenUUID(t *testing.T) {
	assert := assert.New(t)
	for i := 0; i < 100; i++ {
		u := Generate()
		s := u.String()
		assert.EqualValues(s[14], '4')
		u2, err := Parse(s)
		assert.NoError(err)
		assert.Equal(u, u2)
	}
}

// Test uuid.Parse on invalid input.
//
// This test attempts to parse a set of invalid UUIDs.
//
// uuid.Parse should return an error for each invalid UUID.
func TestBadUUID(t *testing.T) {
	badTestUUIDs := []string{
		"",
		"48d9-45d3-8b44-f973e4f35e48",
		"69e8426--ed01-4738-b15f-b47de06b62e7",
		"e35ed972-46c-4aad-a1e7-ef103ae079a2",
		"sba04826-62a5-48bd-876f-9119667b1487",
		"ca957444fa4611e594f938607786d9ec0000",
		"ab68111c-03a6-11e6-87de-001320fb6e31a",
	}

	for _, s := range badTestUUIDs {
		_, err := Parse(s)
		assert.Error(t, err)
	}
}
