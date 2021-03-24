// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"time"

	mutils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/procfs"
)

const namespaceKatashim = "kata_shim"

var (
	rpcDurationsHistogram = prometheus.NewHistogramVec(prometheus.HistogramOpts{
		Namespace: namespaceKatashim,
		Name:      "rpc_durations_histogram_milliseconds",
		Help:      "RPC latency distributions.",
		Buckets:   prometheus.ExponentialBuckets(1, 2, 10),
	},
		[]string{"action"},
	)

	katashimThreads = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: namespaceKatashim,
		Name:      "threads",
		Help:      "Kata containerd shim v2 process threads.",
	})

	katashimProcStatus = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceKatashim,
		Name:      "proc_status",
		Help:      "Kata containerd shim v2 process status.",
	},
		[]string{"item"},
	)

	katashimProcStat = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceKatashim,
		Name:      "proc_stat",
		Help:      "Kata containerd shim v2 process statistics.",
	},
		[]string{"item"},
	)

	katashimNetdev = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceKatashim,
		Name:      "netdev",
		Help:      "Kata containerd shim v2 network devices statistics.",
	},
		[]string{"interface", "item"},
	)

	katashimIOStat = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceKatashim,
		Name:      "io_stat",
		Help:      "Kata containerd shim v2 process IO statistics.",
	},
		[]string{"item"},
	)

	katashimOpenFDs = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: namespaceKatashim,
		Name:      "fds",
		Help:      "Kata containerd shim v2 open FDs.",
	})

	katashimPodOverheadCPU = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: namespaceKatashim,
		Name:      "pod_overhead_cpu",
		Help:      "Kata Pod overhead for CPU resources(percent).",
	})

	katashimPodOverheadMemory = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: namespaceKatashim,
		Name:      "pod_overhead_memory_in_bytes",
		Help:      "Kata Pod overhead for memory resources(bytes).",
	})
)

func registerMetrics() {
	prometheus.MustRegister(rpcDurationsHistogram)
	prometheus.MustRegister(katashimThreads)
	prometheus.MustRegister(katashimProcStatus)
	prometheus.MustRegister(katashimProcStat)
	prometheus.MustRegister(katashimNetdev)
	prometheus.MustRegister(katashimIOStat)
	prometheus.MustRegister(katashimOpenFDs)
	prometheus.MustRegister(katashimPodOverheadCPU)
	prometheus.MustRegister(katashimPodOverheadMemory)
}

// updateShimMetrics will update metrics for kata shim process itself
func updateShimMetrics() error {
	proc, err := procfs.Self()
	if err != nil {
		return err
	}

	// metrics about open FDs
	if fds, err := proc.FileDescriptorsLen(); err == nil {
		katashimOpenFDs.Set(float64(fds))
	}

	// network device metrics
	if netdev, err := proc.NetDev(); err == nil {
		// netdev: map[string]NetDevLine
		for _, v := range netdev {
			mutils.SetGaugeVecNetDev(katashimNetdev, v)
		}
	}

	// proc stat
	if procStat, err := proc.Stat(); err == nil {
		katashimThreads.Set(float64(procStat.NumThreads))
		mutils.SetGaugeVecProcStat(katashimProcStat, procStat)
	}

	// proc status
	if procStatus, err := proc.NewStatus(); err == nil {
		mutils.SetGaugeVecProcStatus(katashimProcStatus, procStatus)
	}

	// porc IO stat
	if ioStat, err := proc.IO(); err == nil {
		mutils.SetGaugeVecProcIO(katashimIOStat, ioStat)
	}

	return nil
}

// statsSandbox returns a detailed sandbox stats.
func (s *service) statsSandbox(ctx context.Context) (vc.SandboxStats, []vc.ContainerStats, error) {
	sandboxStats, err := s.sandbox.Stats(ctx)
	if err != nil {
		return vc.SandboxStats{}, []vc.ContainerStats{}, err
	}

	containerStats := []vc.ContainerStats{}
	for _, c := range s.sandbox.GetAllContainers() {
		cstats, err := s.sandbox.StatsContainer(ctx, c.ID())
		if err != nil {
			return vc.SandboxStats{}, []vc.ContainerStats{}, err
		}
		containerStats = append(containerStats, cstats)
	}

	return sandboxStats, containerStats, nil
}

func calcOverhead(initialSandboxStats, finishSandboxStats vc.SandboxStats, initialContainerStats, finishContainersStats []vc.ContainerStats, deltaTime float64) (float64, float64) {
	hostInitCPU := initialSandboxStats.CgroupStats.CPUStats.CPUUsage.TotalUsage
	guestInitCPU := uint64(0)
	for _, cs := range initialContainerStats {
		guestInitCPU += cs.CgroupStats.CPUStats.CPUUsage.TotalUsage
	}

	hostFinalCPU := finishSandboxStats.CgroupStats.CPUStats.CPUUsage.TotalUsage
	guestFinalCPU := uint64(0)
	for _, cs := range finishContainersStats {
		guestFinalCPU += cs.CgroupStats.CPUStats.CPUUsage.TotalUsage
	}

	var guestMemoryUsage uint64
	for _, cs := range finishContainersStats {
		guestMemoryUsage += cs.CgroupStats.MemoryStats.Usage.Usage
	}

	hostMemoryUsage := finishSandboxStats.CgroupStats.MemoryStats.Usage.Usage

	cpuUsageGuest := float64(guestFinalCPU-guestInitCPU) / deltaTime * 100
	cpuUsageHost := float64(hostFinalCPU-hostInitCPU) / deltaTime * 100

	return float64(hostMemoryUsage - guestMemoryUsage), cpuUsageHost - cpuUsageGuest
}

func (s *service) getPodOverhead(ctx context.Context) (float64, float64, error) {
	initTime := time.Now().UnixNano()
	initialSandboxStats, initialContainerStats, err := s.statsSandbox(ctx)
	if err != nil {
		return 0, 0, err
	}

	// Wait for 1 second to calculate CPU usage
	time.Sleep(time.Second * 1)
	finishtTime := time.Now().UnixNano()
	deltaTime := float64(finishtTime - initTime)

	finishSandboxStats, finishContainersStats, err := s.statsSandbox(ctx)
	if err != nil {
		return 0, 0, err
	}
	mem, cpu := calcOverhead(initialSandboxStats, finishSandboxStats, initialContainerStats, finishContainersStats, deltaTime)
	return mem, cpu, nil
}

func (s *service) setPodOverheadMetrics(ctx context.Context) error {
	mem, cpu, err := s.getPodOverhead(ctx)
	if err != nil {
		return err
	}
	katashimPodOverheadMemory.Set(mem)
	katashimPodOverheadCPU.Set(cpu)
	return nil
}
