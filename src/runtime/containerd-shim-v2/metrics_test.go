// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"testing"

	"github.com/containerd/cgroups"
	"github.com/containerd/containerd/namespaces"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/stretchr/testify/assert"
)

func TestStatNetworkMetric(t *testing.T) {

	assert := assert.New(t)
	var err error

	mockNetwork := []*vc.NetworkStats{
		{
			Name:    "test-network",
			RxBytes: 10,
			TxBytes: 20,
		},
	}

	expectedNetwork := []*cgroups.NetworkStat{
		{
			Name:    "test-network",
			RxBytes: 10,
			TxBytes: 20,
		},
	}

	testingImpl.StatsContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStats, error) {
		return vc.ContainerStats{
			NetworkStats: mockNetwork,
		}, nil
	}

	defer func() {
		testingImpl.StatsContainerFunc = nil
	}()

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	resp, err := testingImpl.StatsContainer(ctx, testSandboxID, testContainerID)
	assert.NoError(err)

	metrics := statsToMetrics(&resp)
	assert.Equal(expectedNetwork, metrics.Network)
}
