// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"

	mutils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/procfs"
)

const namespaceHypervisor = "kata_hypervisor"
const namespaceKatashim = "kata_shim"
const namespaceVirtiofsd = "kata_virtiofsd"

var (
	// hypervisor
	hypervisorThreads = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: namespaceHypervisor,
		Name:      "threads",
		Help:      "Hypervisor process threads.",
	})

	hypervisorProcStatus = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceHypervisor,
		Name:      "proc_status",
		Help:      "Hypervisor process status.",
	},
		[]string{"item"},
	)

	hypervisorProcStat = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceHypervisor,
		Name:      "proc_stat",
		Help:      "Hypervisor process statistics.",
	},
		[]string{"item"},
	)

	hypervisorNetdev = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceHypervisor,
		Name:      "netdev",
		Help:      "Net devices statistics.",
	},
		[]string{"interface", "item"},
	)

	hypervisorIOStat = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceHypervisor,
		Name:      "io_stat",
		Help:      "Process IO statistics.",
	},
		[]string{"item"},
	)

	hypervisorOpenFDs = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: namespaceHypervisor,
		Name:      "fds",
		Help:      "Open FDs for hypervisor.",
	})

	// agent
	agentRPCDurationsHistogram = prometheus.NewHistogramVec(prometheus.HistogramOpts{
		Namespace: namespaceKatashim,
		Name:      "agent_rpc_durations_histogram_milliseconds",
		Help:      "RPC latency distributions.",
		Buckets:   prometheus.ExponentialBuckets(1, 2, 10),
	},
		[]string{"action"},
	)

	// virtiofsd
	virtiofsdThreads = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: namespaceVirtiofsd,
		Name:      "threads",
		Help:      "Virtiofsd process threads.",
	})

	virtiofsdProcStatus = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceVirtiofsd,
		Name:      "proc_status",
		Help:      "Virtiofsd process status.",
	},
		[]string{"item"},
	)

	virtiofsdProcStat = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceVirtiofsd,
		Name:      "proc_stat",
		Help:      "Virtiofsd process statistics.",
	},
		[]string{"item"},
	)

	virtiofsdIOStat = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: namespaceVirtiofsd,
		Name:      "io_stat",
		Help:      "Process IO statistics.",
	},
		[]string{"item"},
	)

	virtiofsdOpenFDs = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: namespaceVirtiofsd,
		Name:      "fds",
		Help:      "Open FDs for virtiofsd.",
	})
)

func RegisterMetrics() {
	// hypervisor
	prometheus.MustRegister(hypervisorThreads)
	prometheus.MustRegister(hypervisorProcStatus)
	prometheus.MustRegister(hypervisorProcStat)
	prometheus.MustRegister(hypervisorNetdev)
	prometheus.MustRegister(hypervisorIOStat)
	prometheus.MustRegister(hypervisorOpenFDs)
	// agent
	prometheus.MustRegister(agentRPCDurationsHistogram)
	// virtiofsd
	prometheus.MustRegister(virtiofsdThreads)
	prometheus.MustRegister(virtiofsdProcStatus)
	prometheus.MustRegister(virtiofsdProcStat)
	prometheus.MustRegister(virtiofsdIOStat)
	prometheus.MustRegister(virtiofsdOpenFDs)
}

// UpdateRuntimeMetrics update shim/hypervisor's metrics
func (s *Sandbox) UpdateRuntimeMetrics() error {
	pids := s.hypervisor.getPids()
	if len(pids) == 0 {
		return nil
	}

	hypervisorPid := pids[0]

	proc, err := procfs.NewProc(hypervisorPid)
	if err != nil {
		return err
	}

	// process FDs
	if fds, err := proc.FileDescriptorsLen(); err == nil {
		hypervisorOpenFDs.Set(float64(fds))
	}

	// process net device statistics
	if netdev, err := proc.NetDev(); err == nil {
		// netdev: map[string]NetDevLine
		for _, v := range netdev {
			mutils.SetGaugeVecNetDev(hypervisorNetdev, v)
		}
	}

	// process statistics
	if procStat, err := proc.Stat(); err == nil {
		hypervisorThreads.Set(float64(procStat.NumThreads))
		mutils.SetGaugeVecProcStat(hypervisorProcStat, procStat)
	}

	// process status
	if procStatus, err := proc.NewStatus(); err == nil {
		mutils.SetGaugeVecProcStatus(hypervisorProcStatus, procStatus)
	}

	// process IO statistics
	if ioStat, err := proc.IO(); err == nil {
		mutils.SetGaugeVecProcIO(hypervisorIOStat, ioStat)
	}

	// virtiofs metrics
	err = s.UpdateVirtiofsdMetrics()
	if err != nil {
		return err
	}

	return nil
}

func (s *Sandbox) UpdateVirtiofsdMetrics() error {
	vfsPid := s.hypervisor.getVirtioFsPid()
	if vfsPid == nil {
		// virtiofsd is not mandatory for a VMM.
		return nil
	}

	proc, err := procfs.NewProc(*vfsPid)
	if err != nil {
		return err
	}

	// process FDs
	if fds, err := proc.FileDescriptorsLen(); err == nil {
		virtiofsdOpenFDs.Set(float64(fds))
	}

	// process statistics
	if procStat, err := proc.Stat(); err == nil {
		virtiofsdThreads.Set(float64(procStat.NumThreads))
		mutils.SetGaugeVecProcStat(virtiofsdProcStat, procStat)
	}

	// process status
	if procStatus, err := proc.NewStatus(); err == nil {
		mutils.SetGaugeVecProcStatus(virtiofsdProcStatus, procStatus)
	}

	// process IO statistics
	if ioStat, err := proc.IO(); err == nil {
		mutils.SetGaugeVecProcIO(virtiofsdIOStat, ioStat)
	}

	return nil
}

func (s *Sandbox) GetAgentMetrics(ctx context.Context) (string, error) {
	r, err := s.agent.getAgentMetrics(ctx, &grpc.GetMetricsRequest{})
	if err != nil {
		return "", err
	}
	return r.Metrics, nil
}
