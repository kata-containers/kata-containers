// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNoProxyStart(t *testing.T) {
	p := &noProxy{}
	assert := assert.New(t)

	agentURL := "agentURL"
	_, _, err := p.start(proxyParams{
		agentURL: agentURL,
	})
	assert.NotNil(err)

	pid, vmURL, err := p.start(proxyParams{
		agentURL: agentURL,
		logger:   testDefaultLogger,
	})
	assert.Nil(err)
	assert.Equal(vmURL, agentURL)
	assert.Equal(pid, 0)

	err = p.stop(0)
	assert.Nil(err)

	assert.False(p.consoleWatched())
}
