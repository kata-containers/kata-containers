// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/procfs"
)

// SetGaugeVecNetDev set gauge for NetDevLine
func SetGaugeVecNetDev(gv *prometheus.GaugeVec, v procfs.NetDevLine) {
	gv.WithLabelValues(v.Name, "recv_bytes").Set(float64(v.RxBytes))
	gv.WithLabelValues(v.Name, "recv_packets").Set(float64(v.RxPackets))
	gv.WithLabelValues(v.Name, "recv_errs").Set(float64(v.RxErrors))
	gv.WithLabelValues(v.Name, "recv_drop").Set(float64(v.RxDropped))
	gv.WithLabelValues(v.Name, "recv_compressed").Set(float64(v.RxCompressed))
	gv.WithLabelValues(v.Name, "recv_fifo").Set(float64(v.RxFIFO))
	gv.WithLabelValues(v.Name, "recv_frame").Set(float64(v.RxFrame))
	gv.WithLabelValues(v.Name, "recv_multicast").Set(float64(v.RxMulticast))

	gv.WithLabelValues(v.Name, "sent_bytes").Set(float64(v.TxBytes))
	gv.WithLabelValues(v.Name, "sent_packets").Set(float64(v.TxPackets))
	gv.WithLabelValues(v.Name, "sent_errs").Set(float64(v.TxErrors))
	gv.WithLabelValues(v.Name, "sent_drop").Set(float64(v.TxDropped))
	gv.WithLabelValues(v.Name, "sent_colls").Set(float64(v.TxCollisions))
	gv.WithLabelValues(v.Name, "sent_carrier").Set(float64(v.TxCarrier))
	gv.WithLabelValues(v.Name, "sent_compressed").Set(float64(v.TxCompressed))
	gv.WithLabelValues(v.Name, "sent_fifo").Set(float64(v.TxFIFO))
}

// SetGaugeVecProcStatus set gauge for ProcStatus
func SetGaugeVecProcStatus(gv *prometheus.GaugeVec, procStatus procfs.ProcStatus) {
	gv.WithLabelValues("vmpeak").Set(float64(procStatus.VmPeak))
	gv.WithLabelValues("vmsize").Set(float64(procStatus.VmSize))
	gv.WithLabelValues("vmlck").Set(float64(procStatus.VmLck))
	gv.WithLabelValues("vmpin").Set(float64(procStatus.VmPin))
	gv.WithLabelValues("vmhwm").Set(float64(procStatus.VmHWM))
	gv.WithLabelValues("vmrss").Set(float64(procStatus.VmRSS))
	gv.WithLabelValues("rssanon").Set(float64(procStatus.RssAnon))
	gv.WithLabelValues("rssfile").Set(float64(procStatus.RssFile))
	gv.WithLabelValues("rssshmem").Set(float64(procStatus.RssShmem))
	gv.WithLabelValues("vmdata").Set(float64(procStatus.VmData))
	gv.WithLabelValues("vmstk").Set(float64(procStatus.VmStk))
	gv.WithLabelValues("vmexe").Set(float64(procStatus.VmExe))
	gv.WithLabelValues("vmlib").Set(float64(procStatus.VmLib))
	gv.WithLabelValues("vmpte").Set(float64(procStatus.VmPTE))
	gv.WithLabelValues("vmpmd").Set(float64(procStatus.VmPMD))
	gv.WithLabelValues("vmswap").Set(float64(procStatus.VmSwap))
	gv.WithLabelValues("hugetlbpages").Set(float64(procStatus.HugetlbPages))
	gv.WithLabelValues("voluntary_ctxt_switches").Set(float64(procStatus.VoluntaryCtxtSwitches))
	gv.WithLabelValues("nonvoluntary_ctxt_switches").Set(float64(procStatus.NonVoluntaryCtxtSwitches))
}

// SetGaugeVecProcIO set gauge for ProcIO
func SetGaugeVecProcIO(gv *prometheus.GaugeVec, ioStat procfs.ProcIO) {
	gv.WithLabelValues("rchar").Set(float64(ioStat.RChar))
	gv.WithLabelValues("wchar").Set(float64(ioStat.WChar))
	gv.WithLabelValues("syscr").Set(float64(ioStat.SyscR))
	gv.WithLabelValues("syscw").Set(float64(ioStat.SyscW))
	gv.WithLabelValues("readbytes").Set(float64(ioStat.ReadBytes))
	gv.WithLabelValues("writebytes").Set(float64(ioStat.WriteBytes))
	gv.WithLabelValues("cancelledwritebytes").Set(float64(ioStat.CancelledWriteBytes))
}

// SetGaugeVecProcStat set gauge for ProcStat
func SetGaugeVecProcStat(gv *prometheus.GaugeVec, procStat procfs.ProcStat) {
	gv.WithLabelValues("utime").Set(float64(procStat.UTime))
	gv.WithLabelValues("stime").Set(float64(procStat.STime))
	gv.WithLabelValues("cutime").Set(float64(procStat.CUTime))
	gv.WithLabelValues("cstime").Set(float64(procStat.CSTime))
}
