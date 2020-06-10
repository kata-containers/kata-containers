// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"

	"github.com/stretchr/testify/assert"
)

func TestServeMetrics(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
	}

	rr := httptest.NewRecorder()
	r := &http.Request{}

	// case 1: normal
	sandbox.GetAgentMetricsFunc = func() (string, error) {
		return `# HELP go_threads Number of OS threads created.
# TYPE go_threads gauge
go_threads 23
`, nil
	}

	defer func() {
		sandbox.GetAgentMetricsFunc = nil
	}()

	s.serveMetrics(rr, r)
	assert.Equal(200, rr.Code, "response code should be 200")
	body := rr.Body.String()

	assert.Equal(true, strings.Contains(body, "kata_agent_go_threads 23\n"))

	// case 2: GetAgentMetricsFunc return error
	sandbox.GetAgentMetricsFunc = func() (string, error) {
		return "", fmt.Errorf("some error occurred")
	}

	s.serveMetrics(rr, r)
	assert.Equal(200, rr.Code, "response code should be 200")
	body = rr.Body.String()
	assert.Equal(true, len(strings.Split(body, "\n")) > 0)
}
