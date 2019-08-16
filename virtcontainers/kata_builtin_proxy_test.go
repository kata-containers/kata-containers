// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

func TestKataBuiltinProxy(t *testing.T) {
	assert := assert.New(t)

	p := kataBuiltInProxy{}

	params := proxyParams{debug: true}

	err := p.validateParams(params)
	assert.NotNil(err)

	params.id = "foobarproxy"
	err = p.validateParams(params)
	assert.NotNil(err)

	params.agentURL = "foobaragent"
	err = p.validateParams(params)
	assert.NotNil(err)

	params.consoleURL = "foobarconsole"
	err = p.validateParams(params)
	assert.Nil(err)

	params.logger = logrus.WithField("proxy", params.id)
	buildinProxyConsoleProto = "foobarproto"
	_, _, err = p.start(params)
	assert.NotNil(err)
	assert.Empty(p.sandboxID)

	err = p.stop(0)
	assert.Nil(err)

	assert.False(p.consoleWatched())
}
