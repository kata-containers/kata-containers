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
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
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

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.StatsContainerFunc = func(contID string) (vc.ContainerStats, error) {
		return vc.ContainerStats{
			NetworkStats: mockNetwork,
		}, nil
	}

	defer func() {
		sandbox.StatsContainerFunc = nil
	}()

	resp, err := sandbox.StatsContainer(context.Background(), testContainerID)
	assert.NoError(err)

	metrics := statsToMetrics(&resp)
	assert.Equal(expectedNetwork, metrics.Network)
}
