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

func newShimAgent(ctx context.Context, addr string) (*shimAgent, error) {
	span, _ := trace(ctx, "newShimAgent")
	defer span.Finish()

	client, err := client.NewAgentClient(ctx, addr, false)
	if err != nil {
		return nil, err
	}

	return &shimAgent{AgentClient: client}, nil
}
