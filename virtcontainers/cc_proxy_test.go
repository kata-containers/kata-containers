// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestCCProxyStart(t *testing.T) {
	proxy := &ccProxy{}

	testProxyStart(t, nil, proxy)
}

func TestCCProxy(t *testing.T) {
	proxy := &ccProxy{}
	assert := assert.New(t)

	err := proxy.stop(0)
	assert.Nil(err)

	assert.False(proxy.consoleWatched())
}
