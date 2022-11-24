// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
package types

import (
	"regexp"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestKataRuntimeNameRegexp(t *testing.T) {
	assert := assert.New(t)

	runtimeNameRegexp, err := regexp.Compile(KataRuntimeNameRegexp)
	assert.NoError(err)

	// valid Kata containers name
	assert.Equal(true, runtimeNameRegexp.MatchString("io.containerd.kata.v2"))
	assert.Equal(true, runtimeNameRegexp.MatchString("io.containerd.kataclh.v2"))
	assert.Equal(true, runtimeNameRegexp.MatchString("io.containerd.kata-clh.v2"))
	assert.Equal(true, runtimeNameRegexp.MatchString("io.containerd.kata.1.2.3-clh.4.v2"))

	// invalid Kata containers name
	assert.Equal(false, runtimeNameRegexp.MatchString("io2containerd.kata.v2"))
	assert.Equal(false, runtimeNameRegexp.MatchString("io.c3ontainerd.kata.v2"))
	assert.Equal(false, runtimeNameRegexp.MatchString("io.containerd.runc.v1"))
}
