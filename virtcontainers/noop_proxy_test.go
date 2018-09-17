// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNoopProxy(t *testing.T) {
	n := &noopProxy{}
	assert := assert.New(t)

	_, url, err := n.start(proxyParams{})
	assert.Nil(err)
	assert.Equal(url, noopProxyURL)

	err = n.stop(0)
	assert.Nil(err)

	assert.False(n.consoleWatched())
}
