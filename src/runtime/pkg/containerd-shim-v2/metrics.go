// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"

	cgroupsv1 "github.com/containerd/cgroups/stats/v1"
	cgroupsv2 "github.com/containerd/cgroups/v2/stats"
	"github.com/containerd/containerd/protobuf"
	resCtrl "github.com/kata-containers/kata-containers/src/runtime/pkg/resourcecontrol"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	anypb "google.golang.org/protobuf/types/known/anypb"
)

func marshalMetrics(ctx context.Context, s *service, containerID string) (*anypb.Any, error) {
	stats, err := s.sandbox.StatsContainer(ctx, containerID)
	if err != nil {
		return nil, err
	}

	isCgroupV1, err := resCtrl.IsCgroupV1()
	if err != nil {
		return nil, err
	}

	var metrics interface{}

	if isCgroupV1 {
		metrics = statsToMetricsV1(&stats)
	} else {
		metrics = statsToMetricsV2(&stats)
	}

	data, err := protobuf.MarshalAnyToProto(metrics)
	if err != nil {
		return nil, err
	}

	return data, nil
}

func statsToMetricsV1(stats *vc.ContainerStats) *cgroupsv1.Metrics {
	metrics := &cgroupsv1.Metrics{}

	if stats.CgroupStats != nil {
		metrics = &cgroupsv1.Metrics{
			Hugetlb: setHugetlbStatsV1(stats.CgroupStats.HugetlbStats),
			Pids:    setPidsStatsV1(stats.CgroupStats.PidsStats),
			CPU:     setCPUStatsV1(stats.CgroupStats.CPUStats),
			Memory:  setMemoryStatsV1(stats.CgroupStats.MemoryStats),
			Blkio:   setBlkioStatsV1(stats.CgroupStats.BlkioStats),
		}
	}
	metrics.Network = setNetworkStats(stats.NetworkStats)

	return metrics
}

func statsToMetricsV2(stats *vc.ContainerStats) *cgroupsv2.Metrics {
	metrics := &cgroupsv2.Metrics{}

	if stats.CgroupStats != nil {
		metrics = &cgroupsv2.Metrics{
			Hugetlb: setHugetlbStatsV2(stats.CgroupStats.HugetlbStats),
			Pids:    setPidsStatsV2(stats.CgroupStats.PidsStats),
			CPU:     setCPUStatsV2(stats.CgroupStats.CPUStats),
			Memory:  setMemoryStatsV2(stats.CgroupStats.MemoryStats),
			Io:      setBlkioStatsV2(stats.CgroupStats.BlkioStats),
		}
	}

	return metrics
}

func setHugetlbStatsV1(vcHugetlb map[string]vc.HugetlbStats) []*cgroupsv1.HugetlbStat {
	var hugetlbStats []*cgroupsv1.HugetlbStat
	for k, v := range vcHugetlb {
		hugetlbStats = append(
			hugetlbStats,
			&cgroupsv1.HugetlbStat{
				Usage:    v.Usage,
				Max:      v.MaxUsage,
				Failcnt:  v.Failcnt,
				Pagesize: k,
			})
	}

	return hugetlbStats
}

func setHugetlbStatsV2(vcHugetlb map[string]vc.HugetlbStats) []*cgroupsv2.HugeTlbStat {
	var hugetlbStats []*cgroupsv2.HugeTlbStat
	for k, v := range vcHugetlb {
		hugetlbStats = append(
			hugetlbStats,
			&cgroupsv2.HugeTlbStat{
				Current:  v.Usage,
				Max:      v.MaxUsage,
				Pagesize: k,
			})
	}

	return hugetlbStats
}

func setPidsStatsV1(vcPids vc.PidsStats) *cgroupsv1.PidsStat {
	pidsStats := &cgroupsv1.PidsStat{
		Current: vcPids.Current,
		Limit:   vcPids.Limit,
	}

	return pidsStats
}

func setPidsStatsV2(vcPids vc.PidsStats) *cgroupsv2.PidsStat {
	pidsStats := &cgroupsv2.PidsStat{
		Current: vcPids.Current,
		Limit:   vcPids.Limit,
	}

	return pidsStats
}

func setCPUStatsV1(vcCPU vc.CPUStats) *cgroupsv1.CPUStat {
	var perCPU []uint64
	perCPU = append(perCPU, vcCPU.CPUUsage.PercpuUsage...)

	cpuStats := &cgroupsv1.CPUStat{
		Usage: &cgroupsv1.CPUUsage{
			Total:  vcCPU.CPUUsage.TotalUsage,
			Kernel: vcCPU.CPUUsage.UsageInKernelmode,
			User:   vcCPU.CPUUsage.UsageInUsermode,
			PerCPU: perCPU,
		},
		Throttling: &cgroupsv1.Throttle{
			Periods:          vcCPU.ThrottlingData.Periods,
			ThrottledPeriods: vcCPU.ThrottlingData.ThrottledPeriods,
			ThrottledTime:    vcCPU.ThrottlingData.ThrottledTime,
		},
	}

	return cpuStats
}

func setCPUStatsV2(vcCPU vc.CPUStats) *cgroupsv2.CPUStat {
	cpuStats := &cgroupsv2.CPUStat{
		UsageUsec:     vcCPU.CPUUsage.TotalUsage / 1000,
		UserUsec:      vcCPU.CPUUsage.UsageInKernelmode / 1000,
		SystemUsec:    vcCPU.CPUUsage.UsageInUsermode / 1000,
		NrPeriods:     vcCPU.ThrottlingData.Periods,
		NrThrottled:   vcCPU.ThrottlingData.ThrottledPeriods,
		ThrottledUsec: vcCPU.ThrottlingData.ThrottledTime / 1000,
	}

	return cpuStats
}

func setMemoryStatsV1(vcMemory vc.MemoryStats) *cgroupsv1.MemoryStat {
	memoryStats := &cgroupsv1.MemoryStat{
		Usage: &cgroupsv1.MemoryEntry{
			Limit:   vcMemory.Usage.Limit,
			Usage:   vcMemory.Usage.Usage,
			Max:     vcMemory.Usage.MaxUsage,
			Failcnt: vcMemory.Usage.Failcnt,
		},
		Swap: &cgroupsv1.MemoryEntry{
			Limit:   vcMemory.SwapUsage.Limit,
			Usage:   vcMemory.SwapUsage.Usage,
			Max:     vcMemory.SwapUsage.MaxUsage,
			Failcnt: vcMemory.SwapUsage.Failcnt,
		},
		Kernel: &cgroupsv1.MemoryEntry{
			Limit:   vcMemory.KernelUsage.Limit,
			Usage:   vcMemory.KernelUsage.Usage,
			Max:     vcMemory.KernelUsage.MaxUsage,
			Failcnt: vcMemory.KernelUsage.Failcnt,
		},
		KernelTCP: &cgroupsv1.MemoryEntry{
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

// nolint: gocyclo
func setMemoryStatsV2(vcMemory vc.MemoryStats) *cgroupsv2.MemoryStat {
	memoryStats := &cgroupsv2.MemoryStat{
		Usage:      vcMemory.Usage.Usage,
		UsageLimit: vcMemory.Usage.Limit,
		SwapUsage:  vcMemory.SwapUsage.Usage,
		SwapLimit:  vcMemory.SwapUsage.Limit,
	}

	if v, ok := vcMemory.Stats["anon"]; ok {
		memoryStats.Anon = v
	}
	if v, ok := vcMemory.Stats["file"]; ok {
		memoryStats.File = v
	}
	if v, ok := vcMemory.Stats["kernel_stack"]; ok {
		memoryStats.KernelStack = v
	}
	if v, ok := vcMemory.Stats["slab"]; ok {
		memoryStats.Slab = v
	}
	if v, ok := vcMemory.Stats["sock"]; ok {
		memoryStats.Sock = v
	}
	if v, ok := vcMemory.Stats["shmem"]; ok {
		memoryStats.Shmem = v
	}
	if v, ok := vcMemory.Stats["file_mapped"]; ok {
		memoryStats.FileMapped = v
	}
	if v, ok := vcMemory.Stats["file_dirty"]; ok {
		memoryStats.FileDirty = v
	}
	if v, ok := vcMemory.Stats["file_writeback"]; ok {
		memoryStats.FileWriteback = v
	}
	if v, ok := vcMemory.Stats["anon_thp"]; ok {
		memoryStats.AnonThp = v
	}
	if v, ok := vcMemory.Stats["inactive_anon"]; ok {
		memoryStats.InactiveAnon = v
	}
	if v, ok := vcMemory.Stats["active_anon"]; ok {
		memoryStats.ActiveAnon = v
	}
	if v, ok := vcMemory.Stats["inactive_file"]; ok {
		memoryStats.InactiveFile = v
	}
	if v, ok := vcMemory.Stats["active_file"]; ok {
		memoryStats.ActiveFile = v
	}
	if v, ok := vcMemory.Stats["unevictable"]; ok {
		memoryStats.Unevictable = v
	}
	if v, ok := vcMemory.Stats["slab_reclaimable"]; ok {
		memoryStats.SlabReclaimable = v
	}
	if v, ok := vcMemory.Stats["slab_unreclaimable"]; ok {
		memoryStats.SlabUnreclaimable = v
	}
	if v, ok := vcMemory.Stats["pgfault"]; ok {
		memoryStats.Pgfault = v
	}
	if v, ok := vcMemory.Stats["pgmajfault"]; ok {
		memoryStats.Pgmajfault = v
	}
	if v, ok := vcMemory.Stats["workingset_refault"]; ok {
		memoryStats.WorkingsetRefault = v
	}
	if v, ok := vcMemory.Stats["workingset_activate"]; ok {
		memoryStats.WorkingsetActivate = v
	}
	if v, ok := vcMemory.Stats["workingset_nodereclaim"]; ok {
		memoryStats.WorkingsetNodereclaim = v
	}
	if v, ok := vcMemory.Stats["pgrefill"]; ok {
		memoryStats.Pgrefill = v
	}
	if v, ok := vcMemory.Stats["pgscan"]; ok {
		memoryStats.Pgscan = v
	}
	if v, ok := vcMemory.Stats["pgsteal"]; ok {
		memoryStats.Pgsteal = v
	}
	if v, ok := vcMemory.Stats["pgactivate"]; ok {
		memoryStats.Pgactivate = v
	}
	if v, ok := vcMemory.Stats["pgdeactivate"]; ok {
		memoryStats.Pgdeactivate = v
	}
	if v, ok := vcMemory.Stats["pglazyfree"]; ok {
		memoryStats.Pglazyfree = v
	}
	if v, ok := vcMemory.Stats["pglazyfreed"]; ok {
		memoryStats.Pglazyfreed = v
	}
	if v, ok := vcMemory.Stats["thp_fault_alloc"]; ok {
		memoryStats.ThpFaultAlloc = v
	}
	if v, ok := vcMemory.Stats["thp_collapse_alloc"]; ok {
		memoryStats.ThpCollapseAlloc = v
	}
	if v, ok := vcMemory.Stats["usage"]; ok {
		memoryStats.Usage = v
	}
	if v, ok := vcMemory.Stats["usage_limit"]; ok {
		memoryStats.UsageLimit = v
	}
	if v, ok := vcMemory.Stats["swap_usage"]; ok {
		memoryStats.SwapUsage = v
	}
	if v, ok := vcMemory.Stats["swap_limit"]; ok {
		memoryStats.SwapLimit = v
	}

	return memoryStats
}

func setBlkioStatsV1(vcBlkio vc.BlkioStats) *cgroupsv1.BlkIOStat {
	blkioStats := &cgroupsv1.BlkIOStat{
		IoServiceBytesRecursive: copyBlkioV1(vcBlkio.IoServiceBytesRecursive),
		IoServicedRecursive:     copyBlkioV1(vcBlkio.IoServicedRecursive),
		IoQueuedRecursive:       copyBlkioV1(vcBlkio.IoQueuedRecursive),
		SectorsRecursive:        copyBlkioV1(vcBlkio.SectorsRecursive),
		IoServiceTimeRecursive:  copyBlkioV1(vcBlkio.IoServiceTimeRecursive),
		IoWaitTimeRecursive:     copyBlkioV1(vcBlkio.IoWaitTimeRecursive),
		IoMergedRecursive:       copyBlkioV1(vcBlkio.IoMergedRecursive),
		IoTimeRecursive:         copyBlkioV1(vcBlkio.IoTimeRecursive),
	}

	return blkioStats
}

func setBlkioStatsV2(vcBlkio vc.BlkioStats) *cgroupsv2.IOStat {
	ioStats := &cgroupsv2.IOStat{
		Usage: copyBlkioV2(vcBlkio.IoServiceBytesRecursive),
	}

	return ioStats
}

func copyBlkioV1(s []vc.BlkioStatEntry) []*cgroupsv1.BlkIOEntry {
	ret := make([]*cgroupsv1.BlkIOEntry, len(s))
	for i, v := range s {
		ret[i] = &cgroupsv1.BlkIOEntry{
			Op:    v.Op,
			Major: v.Major,
			Minor: v.Minor,
			Value: v.Value,
		}
	}

	return ret
}

func copyBlkioV2(s []vc.BlkioStatEntry) []*cgroupsv2.IOEntry {
	var ret []*cgroupsv2.IOEntry
	item := cgroupsv2.IOEntry{}
	for _, v := range s {
		switch v.Op {
		case "read":
			item.Rbytes = v.Value
		case "write":
			item.Wbytes = v.Value
		case "rios":
			item.Rios = v.Value
		case "wios":
			item.Wios = v.Value
		}
		item.Major = v.Major
		item.Minor = v.Minor
	}
	ret = append(ret, &item)

	return ret
}

func setNetworkStats(vcNetwork []*vc.NetworkStats) []*cgroupsv1.NetworkStat {
	networkStats := make([]*cgroupsv1.NetworkStat, len(vcNetwork))
	for i, v := range vcNetwork {
		networkStats[i] = &cgroupsv1.NetworkStat{
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
