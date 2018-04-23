// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"
)

func TestNoProxyStart(t *testing.T) {
	sandbox := &Sandbox{
		agent: newAgent(NoopAgentType),
	}

	p := &noProxy{}

	agentURL := "agentURL"
	pid, vmURL, err := p.start(sandbox, proxyParams{agentURL: agentURL})
	if err != nil {
		t.Fatal(err)
	}

	if vmURL != agentURL {
		t.Fatalf("Got URL %q, expecting %q", vmURL, agentURL)
	}

	if pid != 0 {
		t.Fatal("Failure since returned PID should be 0")
	}
}

func TestNoProxyStop(t *testing.T) {
	p := &noProxy{}

	if err := p.stop(&Sandbox{}, 0); err != nil {
		t.Fatal(err)
	}
}
