// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"os"
	"testing"

	"github.com/containerd/containerd/events"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"

	"github.com/stretchr/testify/assert"
)

func TestNewEventsForwarder(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
	}

	// newEventsForwarder will not call publisher to publish events
	// so here we can use a nil pointer to test newEventsForwarder
	var publisher events.Publisher

	// check log forwarder
	forwarder := s.newEventsForwarder(context.Background(), publisher)
	assert.Equal(forwarderTypeLog, forwarder.forwarderType())

	// check containerd forwarder
	os.Setenv(ttrpcAddressEnv, "/foo/bar.sock")
	defer os.Setenv(ttrpcAddressEnv, "")
	forwarder = s.newEventsForwarder(context.Background(), publisher)
	assert.Equal(forwarderTypeContainerd, forwarder.forwarderType())
}
