// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/kata-containers/agent/protocols/client"
	context "golang.org/x/net/context"
)

type shimAgent struct {
	*client.AgentClient
}

func newShimAgent(addr string) (*shimAgent, error) {
	client, err := client.NewAgentClient(context.Background(), addr, false)
	if err != nil {
		return nil, err
	}

	return &shimAgent{AgentClient: client}, nil
}
