// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"testing"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"

	"github.com/stretchr/testify/assert"
)

func getSandboxCPUFunc(c, m uint64) func() (vc.SandboxStats, error) {
	return func() (vc.SandboxStats, error) {
		return vc.SandboxStats{
			CgroupStats: vc.CgroupStats{
				CPUStats: vc.CPUStats{
					CPUUsage: vc.CPUUsage{
						TotalUsage: c * 1e9,
					},
				},
				MemoryStats: vc.MemoryStats{
					Usage: vc.MemoryData{
						Usage: m,
					},
				},
			},
			Cpus: 20,
		}, nil
	}
}

func getStatsContainerCPUFunc(fooCPU, barCPU, fooMem, barMem uint64) func(contID string) (vc.ContainerStats, error) {
	return func(contID string) (vc.ContainerStats, error) {
		vCPU := fooCPU
		vMem := fooMem
		if contID == "bar" {
			vCPU = barCPU
			vMem = barMem
		}
		return vc.ContainerStats{
			CgroupStats: &vc.CgroupStats{
				CPUStats: vc.CPUStats{
					CPUUsage: vc.CPUUsage{
						TotalUsage: vCPU * 1e9,
					},
				},
				MemoryStats: vc.MemoryStats{
					Usage: vc.MemoryData{
						Usage: vMem,
					},
				},
			},
		}, nil

	}
}

func TestStatsSandbox(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID:             testSandboxID,
		StatsFunc:          getSandboxCPUFunc(1000, 100000),
		StatsContainerFunc: getStatsContainerCPUFunc(100, 200, 10000, 20000),
		MockContainers: []*vcmock.Container{
			{
				MockID: "foo",
			},
			{
				MockID: "bar",
			},
		},
	}

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
	}

	initialSandboxStats, initialContainerStats, err := s.statsSandbox(context.Background())
	assert.Nil(err)
	assert.Equal(uint64(1000*1e9), initialSandboxStats.CgroupStats.CPUStats.CPUUsage.TotalUsage)
	assert.Equal(2, len(initialContainerStats))
	assert.Equal(uint64(100*1e9), initialContainerStats[0].CgroupStats.CPUStats.CPUUsage.TotalUsage)
	assert.Equal(uint64(200*1e9), initialContainerStats[1].CgroupStats.CPUStats.CPUUsage.TotalUsage)
	assert.Equal(uint64(10000), initialContainerStats[0].CgroupStats.MemoryStats.Usage.Usage)
	assert.Equal(uint64(20000), initialContainerStats[1].CgroupStats.MemoryStats.Usage.Usage)

	// get the 2nd stats
	sandbox.StatsFunc = getSandboxCPUFunc(2000, 110000)
	sandbox.StatsContainerFunc = getStatsContainerCPUFunc(200, 400, 20000, 40000)

	finishSandboxStats, finishContainersStats, _ := s.statsSandbox(context.Background())

	// calc overhead
	mem, cpu := calcOverhead(initialSandboxStats, finishSandboxStats, initialContainerStats, finishContainersStats, 1e9)

	// 70000 = (host2.cpu - host1.cpu - (delta containers.1.cpu + delta containers.2.cpu)) * 100
	//       = (2000 - 1000 - (200 -100 + 400 - 200)) * 100
	//       = (1000 - 300) * 100
	//       = 70000
	assert.Equal(float64(70000), cpu)

	// 50000 = 110000 - sum(containers)
	//       = 110000 - (20000 + 40000)
	//       = 50000
	assert.Equal(float64(50000), mem)
}
