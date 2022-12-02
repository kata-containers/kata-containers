//go:build linux

// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"github.com/prometheus/client_golang/prometheus"
)

const fcMetricsNS = "kata_firecracker"

// prometheus metrics Firecracker exposed.
var (
	apiServerMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "api_server",
		Help:      "Metrics related to the internal API server.",
	},
		[]string{"item"},
	)

	blockDeviceMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "block",
		Help:      "Block Device associated metrics.",
	},
		[]string{"item"},
	)

	getRequestsMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "get_api_requests",
		Help:      "Metrics specific to GET API Requests for counting user triggered actions and/or failures.",
	},
		[]string{"item"},
	)

	i8042DeviceMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "i8042",
		Help:      "Metrics specific to the i8042 device.",
	},
		[]string{"item"},
	)

	performanceMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "latencies_us",
		Help:      "Performance metrics related for the moment only to snapshots.",
	},
		[]string{"item"},
	)

	loggerSystemMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "logger",
		Help:      "Metrics for the logging subsystem.",
	},
		[]string{"item"},
	)

	mmdsMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "mmds",
		Help:      "Metrics for the MMDS functionality.",
	},
		[]string{"item"},
	)

	netDeviceMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "net",
		Help:      "Network-related metrics.",
	},
		[]string{"item"},
	)

	patchRequestsMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "patch_api_requests",
		Help:      "Metrics specific to PATCH API Requests for counting user triggered actions and/or failures.",
	},
		[]string{"item"},
	)

	putRequestsMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "put_api_requests",
		Help:      "Metrics specific to PUT API Requests for counting user triggered actions and/or failures.",
	},
		[]string{"item"},
	)

	rTCDeviceMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "rtc",
		Help:      "Metrics specific to the RTC device.",
	},
		[]string{"item"},
	)

	seccompMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "seccomp",
		Help:      "Metrics for the seccomp filtering.",
	},
		[]string{"item"},
	)

	vcpuMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "vcpu",
		Help:      "Metrics specific to VCPUs' mode of functioning.",
	},
		[]string{"item"},
	)

	vmmMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "vmm",
		Help:      "Metrics specific to the machine manager as a whole.",
	},
		[]string{"item"},
	)

	serialDeviceMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "uart",
		Help:      "Metrics specific to the UART device.",
	},
		[]string{"item"},
	)

	signalMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "signals",
		Help:      "Metrics related to signals.",
	},
		[]string{"item"},
	)

	vsockDeviceMetrics = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Namespace: fcMetricsNS,
		Name:      "vsock",
		Help:      "Vsock-related metrics.",
	},
		[]string{"item"},
	)
)

// registerFirecrackerMetrics register all metrics to prometheus.
func registerFirecrackerMetrics() {
	prometheus.MustRegister(apiServerMetrics)
	prometheus.MustRegister(blockDeviceMetrics)
	prometheus.MustRegister(getRequestsMetrics)
	prometheus.MustRegister(i8042DeviceMetrics)
	prometheus.MustRegister(performanceMetrics)
	prometheus.MustRegister(loggerSystemMetrics)
	prometheus.MustRegister(mmdsMetrics)
	prometheus.MustRegister(netDeviceMetrics)
	prometheus.MustRegister(patchRequestsMetrics)
	prometheus.MustRegister(putRequestsMetrics)
	prometheus.MustRegister(rTCDeviceMetrics)
	prometheus.MustRegister(seccompMetrics)
	prometheus.MustRegister(vcpuMetrics)
	prometheus.MustRegister(vmmMetrics)
	prometheus.MustRegister(serialDeviceMetrics)
	prometheus.MustRegister(signalMetrics)
	prometheus.MustRegister(vsockDeviceMetrics)

}

// updateFirecrackerMetrics update all metrics to the latest values.
func updateFirecrackerMetrics(fm *FirecrackerMetrics) {
	// set metrics for APIServerMetrics
	apiServerMetrics.WithLabelValues("process_startup_time_us").Set(float64(fm.APIServer.ProcessStartupTimeUs))
	apiServerMetrics.WithLabelValues("process_startup_time_cpu_us").Set(float64(fm.APIServer.ProcessStartupTimeCPUUs))
	apiServerMetrics.WithLabelValues("sync_response_fails").Set(float64(fm.APIServer.SyncResponseFails))
	apiServerMetrics.WithLabelValues("sync_vmm_send_timeout_count").Set(float64(fm.APIServer.SyncVmmSendTimeoutCount))

	// set metrics for BlockDeviceMetrics
	blockDeviceMetrics.WithLabelValues("activate_fails").Set(float64(fm.Block.ActivateFails))
	blockDeviceMetrics.WithLabelValues("cfg_fails").Set(float64(fm.Block.CfgFails))
	blockDeviceMetrics.WithLabelValues("no_avail_buffer").Set(float64(fm.Block.NoAvailBuffer))
	blockDeviceMetrics.WithLabelValues("event_fails").Set(float64(fm.Block.EventFails))
	blockDeviceMetrics.WithLabelValues("execute_fails").Set(float64(fm.Block.ExecuteFails))
	blockDeviceMetrics.WithLabelValues("invalid_reqs_count").Set(float64(fm.Block.InvalidReqsCount))
	blockDeviceMetrics.WithLabelValues("flush_count").Set(float64(fm.Block.FlushCount))
	blockDeviceMetrics.WithLabelValues("queue_event_count").Set(float64(fm.Block.QueueEventCount))
	blockDeviceMetrics.WithLabelValues("rate_limiter_event_count").Set(float64(fm.Block.RateLimiterEventCount))
	blockDeviceMetrics.WithLabelValues("update_count").Set(float64(fm.Block.UpdateCount))
	blockDeviceMetrics.WithLabelValues("update_fails").Set(float64(fm.Block.UpdateFails))
	blockDeviceMetrics.WithLabelValues("read_bytes").Set(float64(fm.Block.ReadBytes))
	blockDeviceMetrics.WithLabelValues("write_bytes").Set(float64(fm.Block.WriteBytes))
	blockDeviceMetrics.WithLabelValues("read_count").Set(float64(fm.Block.ReadCount))
	blockDeviceMetrics.WithLabelValues("write_count").Set(float64(fm.Block.WriteCount))
	blockDeviceMetrics.WithLabelValues("rate_limiter_throttled_events").Set(float64(fm.Block.RateLimiterThrottledEvents))

	// set metrics for GetRequestsMetrics
	getRequestsMetrics.WithLabelValues("instance_info_count").Set(float64(fm.GetAPIRequests.InstanceInfoCount))
	getRequestsMetrics.WithLabelValues("instance_info_fails").Set(float64(fm.GetAPIRequests.InstanceInfoFails))
	getRequestsMetrics.WithLabelValues("machine_cfg_count").Set(float64(fm.GetAPIRequests.MachineCfgCount))
	getRequestsMetrics.WithLabelValues("machine_cfg_fails").Set(float64(fm.GetAPIRequests.MachineCfgFails))

	// set metrics for I8042DeviceMetrics
	i8042DeviceMetrics.WithLabelValues("error_count").Set(float64(fm.I8042.ErrorCount))
	i8042DeviceMetrics.WithLabelValues("missed_read_count").Set(float64(fm.I8042.MissedReadCount))
	i8042DeviceMetrics.WithLabelValues("missed_write_count").Set(float64(fm.I8042.MissedWriteCount))
	i8042DeviceMetrics.WithLabelValues("read_count").Set(float64(fm.I8042.ReadCount))
	i8042DeviceMetrics.WithLabelValues("reset_count").Set(float64(fm.I8042.ResetCount))
	i8042DeviceMetrics.WithLabelValues("write_count").Set(float64(fm.I8042.WriteCount))

	// set metrics for PerformanceMetrics
	performanceMetrics.WithLabelValues("full_create_snapshot").Set(float64(fm.LatenciesUs.FullCreateSnapshot))
	performanceMetrics.WithLabelValues("diff_create_snapshot").Set(float64(fm.LatenciesUs.DiffCreateSnapshot))
	performanceMetrics.WithLabelValues("load_snapshot").Set(float64(fm.LatenciesUs.LoadSnapshot))
	performanceMetrics.WithLabelValues("pause_vm").Set(float64(fm.LatenciesUs.PauseVM))
	performanceMetrics.WithLabelValues("resume_vm").Set(float64(fm.LatenciesUs.ResumeVM))
	performanceMetrics.WithLabelValues("vmm_full_create_snapshot").Set(float64(fm.LatenciesUs.VmmFullCreateSnapshot))
	performanceMetrics.WithLabelValues("vmm_diff_create_snapshot").Set(float64(fm.LatenciesUs.VmmDiffCreateSnapshot))
	performanceMetrics.WithLabelValues("vmm_load_snapshot").Set(float64(fm.LatenciesUs.VmmLoadSnapshot))
	performanceMetrics.WithLabelValues("vmm_pause_vm").Set(float64(fm.LatenciesUs.VmmPauseVM))
	performanceMetrics.WithLabelValues("vmm_resume_vm").Set(float64(fm.LatenciesUs.VmmResumeVM))

	// set metrics for LoggerSystemMetrics
	loggerSystemMetrics.WithLabelValues("missed_metrics_count").Set(float64(fm.Logger.MissedMetricsCount))
	loggerSystemMetrics.WithLabelValues("metrics_fails").Set(float64(fm.Logger.MetricsFails))
	loggerSystemMetrics.WithLabelValues("missed_log_count").Set(float64(fm.Logger.MissedLogCount))
	loggerSystemMetrics.WithLabelValues("log_fails").Set(float64(fm.Logger.LogFails))

	// set metrics for MmdsMetrics
	mmdsMetrics.WithLabelValues("rx_accepted").Set(float64(fm.Mmds.RxAccepted))
	mmdsMetrics.WithLabelValues("rx_accepted_err").Set(float64(fm.Mmds.RxAcceptedErr))
	mmdsMetrics.WithLabelValues("rx_accepted_unusual").Set(float64(fm.Mmds.RxAcceptedUnusual))
	mmdsMetrics.WithLabelValues("rx_bad_eth").Set(float64(fm.Mmds.RxBadEth))
	mmdsMetrics.WithLabelValues("rx_count").Set(float64(fm.Mmds.RxCount))
	mmdsMetrics.WithLabelValues("tx_bytes").Set(float64(fm.Mmds.TxBytes))
	mmdsMetrics.WithLabelValues("tx_count").Set(float64(fm.Mmds.TxCount))
	mmdsMetrics.WithLabelValues("tx_errors").Set(float64(fm.Mmds.TxErrors))
	mmdsMetrics.WithLabelValues("tx_frames").Set(float64(fm.Mmds.TxFrames))
	mmdsMetrics.WithLabelValues("connections_created").Set(float64(fm.Mmds.ConnectionsCreated))
	mmdsMetrics.WithLabelValues("connections_destroyed").Set(float64(fm.Mmds.ConnectionsDestroyed))

	// set metrics for NetDeviceMetrics
	netDeviceMetrics.WithLabelValues("activate_fails").Set(float64(fm.Net.ActivateFails))
	netDeviceMetrics.WithLabelValues("cfg_fails").Set(float64(fm.Net.CfgFails))
	netDeviceMetrics.WithLabelValues("mac_address_updates").Set(float64(fm.Net.MacAddressUpdates))
	netDeviceMetrics.WithLabelValues("no_rx_avail_buffer").Set(float64(fm.Net.NoRxAvailBuffer))
	netDeviceMetrics.WithLabelValues("no_tx_avail_buffer").Set(float64(fm.Net.NoTxAvailBuffer))
	netDeviceMetrics.WithLabelValues("event_fails").Set(float64(fm.Net.EventFails))
	netDeviceMetrics.WithLabelValues("rx_queue_event_count").Set(float64(fm.Net.RxQueueEventCount))
	netDeviceMetrics.WithLabelValues("rx_event_rate_limiter_count").Set(float64(fm.Net.RxEventRateLimiterCount))
	netDeviceMetrics.WithLabelValues("rx_partial_writes").Set(float64(fm.Net.RxPartialWrites))
	netDeviceMetrics.WithLabelValues("rx_rate_limiter_throttled").Set(float64(fm.Net.RxRateLimiterThrottled))
	netDeviceMetrics.WithLabelValues("rx_tap_event_count").Set(float64(fm.Net.RxTapEventCount))
	netDeviceMetrics.WithLabelValues("rx_bytes_count").Set(float64(fm.Net.RxBytesCount))
	netDeviceMetrics.WithLabelValues("rx_packets_count").Set(float64(fm.Net.RxPacketsCount))
	netDeviceMetrics.WithLabelValues("rx_fails").Set(float64(fm.Net.RxFails))
	netDeviceMetrics.WithLabelValues("rx_count").Set(float64(fm.Net.RxCount))
	netDeviceMetrics.WithLabelValues("tap_read_fails").Set(float64(fm.Net.TapReadFails))
	netDeviceMetrics.WithLabelValues("tap_write_fails").Set(float64(fm.Net.TapWriteFails))
	netDeviceMetrics.WithLabelValues("tx_bytes_count").Set(float64(fm.Net.TxBytesCount))
	netDeviceMetrics.WithLabelValues("tx_malformed_frames").Set(float64(fm.Net.TxMalformedFrames))
	netDeviceMetrics.WithLabelValues("tx_fails").Set(float64(fm.Net.TxFails))
	netDeviceMetrics.WithLabelValues("tx_count").Set(float64(fm.Net.TxCount))
	netDeviceMetrics.WithLabelValues("tx_packets_count").Set(float64(fm.Net.TxPacketsCount))
	netDeviceMetrics.WithLabelValues("tx_partial_reads").Set(float64(fm.Net.TxPartialReads))
	netDeviceMetrics.WithLabelValues("tx_queue_event_count").Set(float64(fm.Net.TxQueueEventCount))
	netDeviceMetrics.WithLabelValues("tx_rate_limiter_event_count").Set(float64(fm.Net.TxRateLimiterEventCount))
	netDeviceMetrics.WithLabelValues("tx_rate_limiter_throttled").Set(float64(fm.Net.TxRateLimiterThrottled))
	netDeviceMetrics.WithLabelValues("tx_spoofed_mac_count").Set(float64(fm.Net.TxSpoofedMacCount))

	// set metrics for PatchRequestsMetrics
	patchRequestsMetrics.WithLabelValues("drive_count").Set(float64(fm.PatchAPIRequests.DriveCount))
	patchRequestsMetrics.WithLabelValues("drive_fails").Set(float64(fm.PatchAPIRequests.DriveFails))
	patchRequestsMetrics.WithLabelValues("network_count").Set(float64(fm.PatchAPIRequests.NetworkCount))
	patchRequestsMetrics.WithLabelValues("network_fails").Set(float64(fm.PatchAPIRequests.NetworkFails))
	patchRequestsMetrics.WithLabelValues("machine_cfg_count").Set(float64(fm.PatchAPIRequests.MachineCfgCount))
	patchRequestsMetrics.WithLabelValues("machine_cfg_fails").Set(float64(fm.PatchAPIRequests.MachineCfgFails))

	// set metrics for PutRequestsMetrics
	putRequestsMetrics.WithLabelValues("actions_count").Set(float64(fm.PutAPIRequests.ActionsCount))
	putRequestsMetrics.WithLabelValues("actions_fails").Set(float64(fm.PutAPIRequests.ActionsFails))
	putRequestsMetrics.WithLabelValues("boot_source_count").Set(float64(fm.PutAPIRequests.BootSourceCount))
	putRequestsMetrics.WithLabelValues("boot_source_fails").Set(float64(fm.PutAPIRequests.BootSourceFails))
	putRequestsMetrics.WithLabelValues("drive_count").Set(float64(fm.PutAPIRequests.DriveCount))
	putRequestsMetrics.WithLabelValues("drive_fails").Set(float64(fm.PutAPIRequests.DriveFails))
	putRequestsMetrics.WithLabelValues("logger_count").Set(float64(fm.PutAPIRequests.LoggerCount))
	putRequestsMetrics.WithLabelValues("logger_fails").Set(float64(fm.PutAPIRequests.LoggerFails))
	putRequestsMetrics.WithLabelValues("machine_cfg_count").Set(float64(fm.PutAPIRequests.MachineCfgCount))
	putRequestsMetrics.WithLabelValues("machine_cfg_fails").Set(float64(fm.PutAPIRequests.MachineCfgFails))
	putRequestsMetrics.WithLabelValues("metrics_count").Set(float64(fm.PutAPIRequests.MetricsCount))
	putRequestsMetrics.WithLabelValues("metrics_fails").Set(float64(fm.PutAPIRequests.MetricsFails))
	putRequestsMetrics.WithLabelValues("network_count").Set(float64(fm.PutAPIRequests.NetworkCount))
	putRequestsMetrics.WithLabelValues("network_fails").Set(float64(fm.PutAPIRequests.NetworkFails))

	// set metrics for RTCDeviceMetrics
	rTCDeviceMetrics.WithLabelValues("error_count").Set(float64(fm.Rtc.ErrorCount))
	rTCDeviceMetrics.WithLabelValues("missed_read_count").Set(float64(fm.Rtc.MissedReadCount))
	rTCDeviceMetrics.WithLabelValues("missed_write_count").Set(float64(fm.Rtc.MissedWriteCount))

	// set metrics for SeccompMetrics
	seccompMetrics.WithLabelValues("num_faults").Set(float64(fm.Seccomp.NumFaults))

	// set metrics for VcpuMetrics
	vcpuMetrics.WithLabelValues("exit_io_in").Set(float64(fm.Vcpu.ExitIoIn))
	vcpuMetrics.WithLabelValues("exit_io_out").Set(float64(fm.Vcpu.ExitIoOut))
	vcpuMetrics.WithLabelValues("exit_mmio_read").Set(float64(fm.Vcpu.ExitMmioRead))
	vcpuMetrics.WithLabelValues("exit_mmio_write").Set(float64(fm.Vcpu.ExitMmioWrite))
	vcpuMetrics.WithLabelValues("failures").Set(float64(fm.Vcpu.Failures))
	vcpuMetrics.WithLabelValues("filter_cpuid").Set(float64(fm.Vcpu.FilterCPUid))

	// set metrics for VmmMetrics
	vmmMetrics.WithLabelValues("device_events").Set(float64(fm.Vmm.DeviceEvents))
	vmmMetrics.WithLabelValues("panic_count").Set(float64(fm.Vmm.PanicCount))

	// set metrics for SerialDeviceMetrics
	serialDeviceMetrics.WithLabelValues("error_count").Set(float64(fm.Uart.ErrorCount))
	serialDeviceMetrics.WithLabelValues("flush_count").Set(float64(fm.Uart.FlushCount))
	serialDeviceMetrics.WithLabelValues("missed_read_count").Set(float64(fm.Uart.MissedReadCount))
	serialDeviceMetrics.WithLabelValues("missed_write_count").Set(float64(fm.Uart.MissedWriteCount))
	serialDeviceMetrics.WithLabelValues("read_count").Set(float64(fm.Uart.ReadCount))
	serialDeviceMetrics.WithLabelValues("write_count").Set(float64(fm.Uart.WriteCount))

	// set metrics for SignalMetrics
	signalMetrics.WithLabelValues("sigbus").Set(float64(fm.Signals.Sigbus))
	signalMetrics.WithLabelValues("sigsegv").Set(float64(fm.Signals.Sigsegv))

	// set metrics for VsockDeviceMetrics
	vsockDeviceMetrics.WithLabelValues("activate_fails").Set(float64(fm.Vsock.ActivateFails))
	vsockDeviceMetrics.WithLabelValues("cfg_fails").Set(float64(fm.Vsock.CfgFails))
	vsockDeviceMetrics.WithLabelValues("rx_queue_event_fails").Set(float64(fm.Vsock.RxQueueEventFails))
	vsockDeviceMetrics.WithLabelValues("tx_queue_event_fails").Set(float64(fm.Vsock.TxQueueEventFails))
	vsockDeviceMetrics.WithLabelValues("ev_queue_event_fails").Set(float64(fm.Vsock.EvQueueEventFails))
	vsockDeviceMetrics.WithLabelValues("muxer_event_fails").Set(float64(fm.Vsock.MuxerEventFails))
	vsockDeviceMetrics.WithLabelValues("conn_event_fails").Set(float64(fm.Vsock.ConnEventFails))
	vsockDeviceMetrics.WithLabelValues("rx_queue_event_count").Set(float64(fm.Vsock.RxQueueEventCount))
	vsockDeviceMetrics.WithLabelValues("tx_queue_event_count").Set(float64(fm.Vsock.TxQueueEventCount))
	vsockDeviceMetrics.WithLabelValues("rx_bytes_count").Set(float64(fm.Vsock.RxBytesCount))
	vsockDeviceMetrics.WithLabelValues("tx_bytes_count").Set(float64(fm.Vsock.TxBytesCount))
	vsockDeviceMetrics.WithLabelValues("rx_packets_count").Set(float64(fm.Vsock.RxPacketsCount))
	vsockDeviceMetrics.WithLabelValues("tx_packets_count").Set(float64(fm.Vsock.TxPacketsCount))
	vsockDeviceMetrics.WithLabelValues("conns_added").Set(float64(fm.Vsock.ConnsAdded))
	vsockDeviceMetrics.WithLabelValues("conns_killed").Set(float64(fm.Vsock.ConnsKilled))
	vsockDeviceMetrics.WithLabelValues("conns_removed").Set(float64(fm.Vsock.ConnsRemoved))
	vsockDeviceMetrics.WithLabelValues("killq_resync").Set(float64(fm.Vsock.KillqResync))
	vsockDeviceMetrics.WithLabelValues("tx_flush_fails").Set(float64(fm.Vsock.TxFlushFails))
	vsockDeviceMetrics.WithLabelValues("tx_write_fails").Set(float64(fm.Vsock.TxWriteFails))
	vsockDeviceMetrics.WithLabelValues("rx_read_fails").Set(float64(fm.Vsock.RxReadFails))

}

// Structure storing all metrics while enforcing serialization support on them.
type FirecrackerMetrics struct {
	// API Server related metrics.
	APIServer APIServerMetrics `json:"api_server"`
	// A block device's related metrics.
	Block BlockDeviceMetrics `json:"block"`
	// Metrics related to API GET requests.
	GetAPIRequests GetRequestsMetrics `json:"get_api_requests"`
	// Metrics related to the i8042 device.
	I8042 I8042DeviceMetrics `json:"i8042"`
	// Metrics related to performance measurements.
	LatenciesUs PerformanceMetrics `json:"latencies_us"`
	// Logging related metrics.
	Logger LoggerSystemMetrics `json:"logger"`
	// Metrics specific to MMDS functionality.
	Mmds MmdsMetrics `json:"mmds"`
	// A network device's related metrics.
	Net NetDeviceMetrics `json:"net"`
	// Metrics related to API PATCH requests.
	PatchAPIRequests PatchRequestsMetrics `json:"patch_api_requests"`
	// Metrics related to API PUT requests.
	PutAPIRequests PutRequestsMetrics `json:"put_api_requests"`
	// Metrics related to the RTC device.
	Rtc RTCDeviceMetrics `json:"rtc"`
	// Metrics related to seccomp filtering.
	Seccomp SeccompMetrics `json:"seccomp"`
	// Metrics related to a vcpu's functioning.
	Vcpu VcpuMetrics `json:"vcpu"`
	// Metrics related to the virtual machine manager.
	Vmm VmmMetrics `json:"vmm"`
	// Metrics related to the UART device.
	Uart SerialDeviceMetrics `json:"uart"`
	// Metrics related to signals.
	Signals SignalMetrics `json:"signals"`
	// Metrics related to virtio-vsockets.
	Vsock VsockDeviceMetrics `json:"vsock"`
}

// API Server related metrics.
type APIServerMetrics struct {
	// Measures the process's startup time in microseconds.
	ProcessStartupTimeUs uint64 `json:"process_startup_time_us"`
	// Measures the cpu's startup time in microseconds.
	ProcessStartupTimeCPUUs uint64 `json:"process_startup_time_cpu_us"`
	// Number of failures on API requests triggered by internal errors.
	SyncResponseFails uint64 `json:"sync_response_fails"`
	// Number of timeouts during communication with the VMM.
	SyncVmmSendTimeoutCount uint64 `json:"sync_vmm_send_timeout_count"`
}

// A block device's related metrics.
type BlockDeviceMetrics struct {
	// Number of times when activate failed on a block device.
	ActivateFails uint64 `json:"activate_fails"`
	// Number of times when interacting with the space config of a block device failed.
	CfgFails uint64 `json:"cfg_fails"`
	// No available buffer for the block queue.
	NoAvailBuffer uint64 `json:"no_avail_buffer"`
	// Number of times when handling events on a block device failed.
	EventFails uint64 `json:"event_fails"`
	// Number of failures in executing a request on a block device.
	ExecuteFails uint64 `json:"execute_fails"`
	// Number of invalid requests received for this block device.
	InvalidReqsCount uint64 `json:"invalid_reqs_count"`
	// Number of flushes operation triggered on this block device.
	FlushCount uint64 `json:"flush_count"`
	// Number of events triggerd on the queue of this block device.
	QueueEventCount uint64 `json:"queue_event_count"`
	// Number of events ratelimiter-related.
	RateLimiterEventCount uint64 `json:"rate_limiter_event_count"`
	// Number of update operation triggered on this block device.
	UpdateCount uint64 `json:"update_count"`
	// Number of failures while doing update on this block device.
	UpdateFails uint64 `json:"update_fails"`
	// Number of bytes read by this block device.
	ReadBytes uint64 `json:"read_bytes"`
	// Number of bytes written by this block device.
	WriteBytes uint64 `json:"write_bytes"`
	// Number of successful read operations.
	ReadCount uint64 `json:"read_count"`
	// Number of successful write operations.
	WriteCount uint64 `json:"write_count"`
	// Number of rate limiter throttling events.
	RateLimiterThrottledEvents uint64 `json:"rate_limiter_throttled_events"`
}

// Metrics related to API GET requests.
type GetRequestsMetrics struct {
	// Number of GETs for getting information on the instance.
	InstanceInfoCount uint64 `json:"instance_info_count"`
	// Number of failures when obtaining information on the current instance.
	InstanceInfoFails uint64 `json:"instance_info_fails"`
	// Number of GETs for getting status on attaching machine configuration.
	MachineCfgCount uint64 `json:"machine_cfg_count"`
	// Number of failures during GETs for getting information on the instance.
	MachineCfgFails uint64 `json:"machine_cfg_fails"`
}

// Metrics related to the i8042 device.
type I8042DeviceMetrics struct {
	// Errors triggered while using the i8042 device.
	ErrorCount uint64 `json:"error_count"`
	// Number of superfluous read intents on this i8042 device.
	MissedReadCount uint64 `json:"missed_read_count"`
	// Number of superfluous write intents on this i8042 device.
	MissedWriteCount uint64 `json:"missed_write_count"`
	// Bytes read by this device.
	ReadCount uint64 `json:"read_count"`
	// Number of resets done by this device.
	ResetCount uint64 `json:"reset_count"`
	// Bytes written by this device.
	WriteCount uint64 `json:"write_count"`
}

// Metrics related to performance measurements.
type PerformanceMetrics struct {
	// Measures the snapshot full create time, at the API (user) level, in microseconds.
	FullCreateSnapshot uint64 `json:"full_create_snapshot"`
	// Measures the snapshot diff create time, at the API (user) level, in microseconds.
	DiffCreateSnapshot uint64 `json:"diff_create_snapshot"`
	// Measures the snapshot Load time, at the API (user) level, in microseconds.
	LoadSnapshot uint64 `json:"load_snapshot"`
	// Measures the microVM pausing duration, at the API (user) level, in microseconds.
	PauseVM uint64 `json:"pause_vm"`
	// Measures the microVM resuming duration, at the API (user) level, in microseconds.
	ResumeVM uint64 `json:"resume_vm"`
	// Measures the snapshot full create time, at the VMM level, in microseconds.
	VmmFullCreateSnapshot uint64 `json:"vmm_full_create_snapshot"`
	// Measures the snapshot diff create time, at the VMM level, in microseconds.
	VmmDiffCreateSnapshot uint64 `json:"vmm_diff_create_snapshot"`
	// Measures the snapshot Load time, at the VMM level, in microseconds.
	VmmLoadSnapshot uint64 `json:"vmm_load_snapshot"`
	// Measures the microVM pausing duration, at the VMM level, in microseconds.
	VmmPauseVM uint64 `json:"vmm_pause_vm"`
	// Measures the microVM resuming duration, at the VMM level, in microseconds.
	VmmResumeVM uint64 `json:"vmm_resume_vm"`
}

// Logging related metrics.
type LoggerSystemMetrics struct {
	// Number of misses on flushing metrics.
	MissedMetricsCount uint64 `json:"missed_metrics_count"`
	// Number of errors during metrics handling.
	MetricsFails uint64 `json:"metrics_fails"`
	// Number of misses on logging human readable content.
	MissedLogCount uint64 `json:"missed_log_count"`
	// Number of errors while trying to log human readable content.
	LogFails uint64 `json:"log_fails"`
}

// Metrics specific to MMDS functionality.
type MmdsMetrics struct {
	// Number of frames rerouted to MMDS.
	RxAccepted uint64 `json:"rx_accepted"`
	// Number of errors while handling a frame through MMDS.
	RxAcceptedErr uint64 `json:"rx_accepted_err"`
	// Number of uncommon events encountered while processing packets through MMDS.
	RxAcceptedUnusual uint64 `json:"rx_accepted_unusual"`
	// The number of buffers which couldn't be parsed as valid Ethernet frames by the MMDS.
	RxBadEth uint64 `json:"rx_bad_eth"`
	// The total number of successful receive operations by the MMDS.
	RxCount uint64 `json:"rx_count"`
	// The total number of bytes sent by the MMDS.
	TxBytes uint64 `json:"tx_bytes"`
	// The total number of successful send operations by the MMDS.
	TxCount uint64 `json:"tx_count"`
	// The number of errors raised by the MMDS while attempting to send frames/packets/segments.
	TxErrors uint64 `json:"tx_errors"`
	// The number of frames sent by the MMDS.
	TxFrames uint64 `json:"tx_frames"`
	// The number of connections successfully accepted by the MMDS TCP handler.
	ConnectionsCreated uint64 `json:"connections_created"`
	// The number of connections cleaned up by the MMDS TCP handler.
	ConnectionsDestroyed uint64 `json:"connections_destroyed"`
}

// A network device's related metrics.
type NetDeviceMetrics struct {
	// Number of times when activate failed on a network device.
	ActivateFails uint64 `json:"activate_fails"`
	// Number of times when interacting with the space config of a network device failed.
	CfgFails          uint64 `json:"cfg_fails"`
	MacAddressUpdates uint64 `json:"mac_address_updates"`
	// No available buffer for the net device rx queue.
	NoRxAvailBuffer uint64 `json:"no_rx_avail_buffer"`
	// No available buffer for the net device tx queue.
	NoTxAvailBuffer uint64 `json:"no_tx_avail_buffer"`
	// Number of times when handling events on a network device failed.
	EventFails uint64 `json:"event_fails"`
	// Number of events associated with the receiving queue.
	RxQueueEventCount uint64 `json:"rx_queue_event_count"`
	// Number of events associated with the rate limiter installed on the receiving path.
	RxEventRateLimiterCount uint64 `json:"rx_event_rate_limiter_count"`
	// Number of RX partial writes to guest.
	RxPartialWrites uint64 `json:"rx_partial_writes"`
	// Number of RX rate limiter throttling events.
	RxRateLimiterThrottled uint64 `json:"rx_rate_limiter_throttled"`
	// Number of events received on the associated tap.
	RxTapEventCount uint64 `json:"rx_tap_event_count"`
	// Number of bytes received.
	RxBytesCount uint64 `json:"rx_bytes_count"`
	// Number of packets received.
	RxPacketsCount uint64 `json:"rx_packets_count"`
	// Number of errors while receiving data.
	RxFails uint64 `json:"rx_fails"`
	// Number of successful read operations while receiving data.
	RxCount uint64 `json:"rx_count"`
	// Number of times reading from TAP failed.
	TapReadFails uint64 `json:"tap_read_fails"`
	// Number of times writing to TAP failed.
	TapWriteFails uint64 `json:"tap_write_fails"`
	// Number of transmitted bytes.
	TxBytesCount uint64 `json:"tx_bytes_count"`
	// Number of malformed TX frames.
	TxMalformedFrames uint64 `json:"tx_malformed_frames"`
	// Number of errors while transmitting data.
	TxFails uint64 `json:"tx_fails"`
	// Number of successful write operations while transmitting data.
	TxCount uint64 `json:"tx_count"`
	// Number of transmitted packets.
	TxPacketsCount uint64 `json:"tx_packets_count"`
	// Number of TX partial reads from guest.
	TxPartialReads uint64 `json:"tx_partial_reads"`
	// Number of events associated with the transmitting queue.
	TxQueueEventCount uint64 `json:"tx_queue_event_count"`
	// Number of events associated with the rate limiter installed on the transmitting path.
	TxRateLimiterEventCount uint64 `json:"tx_rate_limiter_event_count"`
	// Number of RX rate limiter throttling events.
	TxRateLimiterThrottled uint64 `json:"tx_rate_limiter_throttled"`
	// Number of packets with a spoofed mac, sent by the guest.
	TxSpoofedMacCount uint64 `json:"tx_spoofed_mac_count"`
}

// Metrics related to API PATCH requests.
type PatchRequestsMetrics struct {
	// Number of tries to PATCH a block device.
	DriveCount uint64 `json:"drive_count"`
	// Number of failures in PATCHing a block device.
	DriveFails uint64 `json:"drive_fails"`
	// Number of tries to PATCH a net device.
	NetworkCount uint64 `json:"network_count"`
	// Number of failures in PATCHing a net device.
	NetworkFails uint64 `json:"network_fails"`
	// Number of PATCHs for configuring the machine.
	MachineCfgCount uint64 `json:"machine_cfg_count"`
	// Number of failures in configuring the machine.
	MachineCfgFails uint64 `json:"machine_cfg_fails"`
}

// Metrics related to API PUT requests.
type PutRequestsMetrics struct {
	// Number of PUTs triggering an action on the VM.
	ActionsCount uint64 `json:"actions_count"`
	// Number of failures in triggering an action on the VM.
	ActionsFails uint64 `json:"actions_fails"`
	// Number of PUTs for attaching source of boot.
	BootSourceCount uint64 `json:"boot_source_count"`
	// Number of failures during attaching source of boot.
	BootSourceFails uint64 `json:"boot_source_fails"`
	// Number of PUTs triggering a block attach.
	DriveCount uint64 `json:"drive_count"`
	// Number of failures in attaching a block device.
	DriveFails uint64 `json:"drive_fails"`
	// Number of PUTs for initializing the logging system.
	LoggerCount uint64 `json:"logger_count"`
	// Number of failures in initializing the logging system.
	LoggerFails uint64 `json:"logger_fails"`
	// Number of PUTs for configuring the machine.
	MachineCfgCount uint64 `json:"machine_cfg_count"`
	// Number of failures in configuring the machine.
	MachineCfgFails uint64 `json:"machine_cfg_fails"`
	// Number of PUTs for initializing the metrics system.
	MetricsCount uint64 `json:"metrics_count"`
	// Number of failures in initializing the metrics system.
	MetricsFails uint64 `json:"metrics_fails"`
	// Number of PUTs for creating a new network interface.
	NetworkCount uint64 `json:"network_count"`
	// Number of failures in creating a new network interface.
	NetworkFails uint64 `json:"network_fails"`
}

// Metrics related to the RTC device.
type RTCDeviceMetrics struct {
	// Errors triggered while using the RTC device.
	ErrorCount uint64 `json:"error_count"`
	// Number of superfluous read intents on this RTC device.
	MissedReadCount uint64 `json:"missed_read_count"`
	// Number of superfluous write intents on this RTC device.
	MissedWriteCount uint64 `json:"missed_write_count"`
}

// Metrics related to seccomp filtering.
type SeccompMetrics struct {
	// Number of errors inside the seccomp filtering.
	NumFaults uint64 `json:"num_faults"`
}

// Metrics related to a vcpu's functioning.
type VcpuMetrics struct {
	// Number of KVM exits for handling input IO.
	ExitIoIn uint64 `json:"exit_io_in"`
	// Number of KVM exits for handling output IO.
	ExitIoOut uint64 `json:"exit_io_out"`
	// Number of KVM exits for handling MMIO reads.
	ExitMmioRead uint64 `json:"exit_mmio_read"`
	// Number of KVM exits for handling MMIO writes.
	ExitMmioWrite uint64 `json:"exit_mmio_write"`
	// Number of errors during this VCPU's run.
	Failures uint64 `json:"failures"`
	// Failures in configuring the CPUID.
	FilterCPUid uint64 `json:"filter_cpuid"`
}

// Metrics related to the virtual machine manager.
type VmmMetrics struct {
	// Number of device related events received for a VM.
	DeviceEvents uint64 `json:"device_events"`
	// Metric for signaling a panic has occurred.
	PanicCount uint64 `json:"panic_count"`
}

// Metrics related to the UART device.
type SerialDeviceMetrics struct {
	// Errors triggered while using the UART device.
	ErrorCount uint64 `json:"error_count"`
	// Number of flush operations.
	FlushCount uint64 `json:"flush_count"`
	// Number of read calls that did not trigger a read.
	MissedReadCount uint64 `json:"missed_read_count"`
	// Number of write calls that did not trigger a write.
	MissedWriteCount uint64 `json:"missed_write_count"`
	// Number of succeeded read calls.
	ReadCount uint64 `json:"read_count"`
	// Number of succeeded write calls.
	WriteCount uint64 `json:"write_count"`
}

// Metrics related to signals.
type SignalMetrics struct {
	// Number of times that SIGBUS was handled.
	Sigbus uint64 `json:"sigbus"`
	// Number of times that SIGSEGV was handled.
	Sigsegv uint64 `json:"sigsegv"`
}

// Metrics related to virtio-vsockets.
type VsockDeviceMetrics struct {
	// Number of times when activate failed on a vsock device.
	ActivateFails uint64 `json:"activate_fails"`
	// Number of times when interacting with the space config of a vsock device failed.
	CfgFails uint64 `json:"cfg_fails"`
	// Number of times when handling RX queue events on a vsock device failed.
	RxQueueEventFails uint64 `json:"rx_queue_event_fails"`
	// Number of times when handling TX queue events on a vsock device failed.
	TxQueueEventFails uint64 `json:"tx_queue_event_fails"`
	// Number of times when handling event queue events on a vsock device failed.
	EvQueueEventFails uint64 `json:"ev_queue_event_fails"`
	// Number of times when handling muxer events on a vsock device failed.
	MuxerEventFails uint64 `json:"muxer_event_fails"`
	// Number of times when handling connection events on a vsock device failed.
	ConnEventFails uint64 `json:"conn_event_fails"`
	// Number of events associated with the receiving queue.
	RxQueueEventCount uint64 `json:"rx_queue_event_count"`
	// Number of events associated with the transmitting queue.
	TxQueueEventCount uint64 `json:"tx_queue_event_count"`
	// Number of bytes received.
	RxBytesCount uint64 `json:"rx_bytes_count"`
	// Number of transmitted bytes.
	TxBytesCount uint64 `json:"tx_bytes_count"`
	// Number of packets received.
	RxPacketsCount uint64 `json:"rx_packets_count"`
	// Number of transmitted packets.
	TxPacketsCount uint64 `json:"tx_packets_count"`
	// Number of added connections.
	ConnsAdded uint64 `json:"conns_added"`
	// Number of killed connections.
	ConnsKilled uint64 `json:"conns_killed"`
	// Number of removed connections.
	ConnsRemoved uint64 `json:"conns_removed"`
	// How many times the killq has been resynced.
	KillqResync uint64 `json:"killq_resync"`
	// How many flush fails have been seen.
	TxFlushFails uint64 `json:"tx_flush_fails"`
	// How many write fails have been seen.
	TxWriteFails uint64 `json:"tx_write_fails"`
	// Number of times read() has failed.
	RxReadFails uint64 `json:"rx_read_fails"`
}
