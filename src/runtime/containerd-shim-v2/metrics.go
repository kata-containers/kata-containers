// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"github.com/containerd/cgroups"
	"github.com/containerd/typeurl"

	google_protobuf "github.com/gogo/protobuf/types"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
)

func marshalMetrics(s *service, containerID string) (*google_protobuf.Any, error) {
	stats, err := s.sandbox.StatsContainer(containerID)
	if err != nil {
		return nil, err
	}

	metrics := statsToMetrics(&stats)

	data, err := typeurl.MarshalAny(metrics)
	if err != nil {
		return nil, err
	}

	return data, nil
}

func statsToMetrics(stats *vc.ContainerStats) *cgroups.Metrics {
	metrics := &cgroups.Metrics{}

	if stats.CgroupStats != nil {
		metrics = &cgroups.Metrics{
			Hugetlb: setHugetlbStats(stats.CgroupStats.HugetlbStats),
			Pids:    setPidsStats(stats.CgroupStats.PidsStats),
			CPU:     setCPUStats(stats.CgroupStats.CPUStats),
			Memory:  setMemoryStats(stats.CgroupStats.MemoryStats),
			Blkio:   setBlkioStats(stats.CgroupStats.BlkioStats),
		}
	}

	metrics.Network = setNetworkStats(stats.NetworkStats)

	return metrics
}

func setHugetlbStats(vcHugetlb map[string]vc.HugetlbStats) []*cgroups.HugetlbStat {
	var hugetlbStats []*cgroups.HugetlbStat
	for _, v := range vcHugetlb {
		hugetlbStats = append(
			hugetlbStats,
			&cgroups.HugetlbStat{
				Usage:   v.Usage,
				Max:     v.MaxUsage,
				Failcnt: v.Failcnt,
			})
	}

	return hugetlbStats
}

func setPidsStats(vcPids vc.PidsStats) *cgroups.PidsStat {
	pidsStats := &cgroups.PidsStat{
		Current: vcPids.Current,
		Limit:   vcPids.Limit,
	}

	return pidsStats
}

func setCPUStats(vcCPU vc.CPUStats) *cgroups.CPUStat {

	var perCPU []uint64
	perCPU = append(perCPU, vcCPU.CPUUsage.PercpuUsage...)

	cpuStats := &cgroups.CPUStat{
		Usage: &cgroups.CPUUsage{
			Total:  vcCPU.CPUUsage.TotalUsage,
			Kernel: vcCPU.CPUUsage.UsageInKernelmode,
			User:   vcCPU.CPUUsage.UsageInUsermode,
			PerCPU: perCPU,
		},
		Throttling: &cgroups.Throttle{
			Periods:          vcCPU.ThrottlingData.Periods,
			ThrottledPeriods: vcCPU.ThrottlingData.ThrottledPeriods,
			ThrottledTime:    vcCPU.ThrottlingData.ThrottledTime,
		},
	}

	return cpuStats
}

func setMemoryStats(vcMemory vc.MemoryStats) *cgroups.MemoryStat {
	memoryStats := &cgroups.MemoryStat{
		Usage: &cgroups.MemoryEntry{
			Limit:   vcMemory.Usage.Limit,
			Usage:   vcMemory.Usage.Usage,
			Max:     vcMemory.Usage.MaxUsage,
			Failcnt: vcMemory.Usage.Failcnt,
		},
		Swap: &cgroups.MemoryEntry{
			Limit:   vcMemory.SwapUsage.Limit,
			Usage:   vcMemory.SwapUsage.Usage,
			Max:     vcMemory.SwapUsage.MaxUsage,
			Failcnt: vcMemory.SwapUsage.Failcnt,
		},
		Kernel: &cgroups.MemoryEntry{
			Limit:   vcMemory.KernelUsage.Limit,
			Usage:   vcMemory.KernelUsage.Usage,
			Max:     vcMemory.KernelUsage.MaxUsage,
			Failcnt: vcMemory.KernelUsage.Failcnt,
		},
		KernelTCP: &cgroups.MemoryEntry{
			Limit:   vcMemory.KernelTCPUsage.Limit,
			Usage:   vcMemory.KernelTCPUsage.Usage,
			Max:     vcMemory.KernelTCPUsage.MaxUsage,
			Failcnt: vcMemory.KernelTCPUsage.Failcnt,
		},
	}

	if vcMemory.UseHierarchy {
		memoryStats.Cache = vcMemory.Stats["total_cache"]
		memoryStats.RSS = vcMemory.Stats["total_rss"]
		memoryStats.MappedFile = vcMemory.Stats["total_mapped_file"]
	} else {
		memoryStats.Cache = vcMemory.Stats["cache"]
		memoryStats.RSS = vcMemory.Stats["rss"]
		memoryStats.MappedFile = vcMemory.Stats["mapped_file"]
	}
	if v, ok := vcMemory.Stats["pgfault"]; ok {
		memoryStats.PgFault = v
	}
	if v, ok := vcMemory.Stats["pgmajfault"]; ok {
		memoryStats.PgMajFault = v
	}
	if v, ok := vcMemory.Stats["total_inactive_file"]; ok {
		memoryStats.TotalInactiveFile = v
	}

	return memoryStats
}

func setBlkioStats(vcBlkio vc.BlkioStats) *cgroups.BlkIOStat {
	blkioStats := &cgroups.BlkIOStat{
		IoServiceBytesRecursive: copyBlkio(vcBlkio.IoServiceBytesRecursive),
		IoServicedRecursive:     copyBlkio(vcBlkio.IoServicedRecursive),
		IoQueuedRecursive:       copyBlkio(vcBlkio.IoQueuedRecursive),
		SectorsRecursive:        copyBlkio(vcBlkio.SectorsRecursive),
		IoServiceTimeRecursive:  copyBlkio(vcBlkio.IoServiceTimeRecursive),
		IoWaitTimeRecursive:     copyBlkio(vcBlkio.IoWaitTimeRecursive),
		IoMergedRecursive:       copyBlkio(vcBlkio.IoMergedRecursive),
		IoTimeRecursive:         copyBlkio(vcBlkio.IoTimeRecursive),
	}

	return blkioStats
}

func copyBlkio(s []vc.BlkioStatEntry) []*cgroups.BlkIOEntry {
	ret := make([]*cgroups.BlkIOEntry, len(s))
	for i, v := range s {
		ret[i] = &cgroups.BlkIOEntry{
			Op:    v.Op,
			Major: v.Major,
			Minor: v.Minor,
			Value: v.Value,
		}
	}

	return ret
}

func setNetworkStats(vcNetwork []*vc.NetworkStats) []*cgroups.NetworkStat {
	networkStats := make([]*cgroups.NetworkStat, len(vcNetwork))
	for i, v := range vcNetwork {
		networkStats[i] = &cgroups.NetworkStat{
			Name:      v.Name,
			RxBytes:   v.RxBytes,
			RxPackets: v.RxPackets,
			RxErrors:  v.RxErrors,
			RxDropped: v.RxDropped,
			TxBytes:   v.TxBytes,
			TxPackets: v.TxPackets,
			TxErrors:  v.TxErrors,
			TxDropped: v.TxDropped,
		}
	}

	return networkStats
}
