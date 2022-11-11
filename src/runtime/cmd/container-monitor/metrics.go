package main

import (
	"fmt"

	v1 "github.com/containerd/containerd/metrics/types/v1"
	metrics "github.com/docker/go-metrics"
	"github.com/prometheus/client_golang/prometheus"
)

type metric struct {
	name   string
	help   string
	unit   metrics.Unit
	vt     prometheus.ValueType
	labels []string
	// getvalues returns the value and labels for the data
	getValues func(stats *v1.Metrics) value
}

type value struct {
	v float64
	l []string
}

func (m *metric) desc() *prometheus.Desc {
	name := m.name
	if m.unit != "" {
		name = fmt.Sprintf("%s_%s", m.name, m.unit)
	}
	return prometheus.NewDesc(name, m.help, append([]string{"container", "namespace", "pod"}, m.labels...), make(map[string]string))
}

var cpuMetrics = []*metric{
	{
		name: "cpu_total",
		help: "The total cpu time",
		unit: metrics.Nanoseconds,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.CPU == nil {
				return value{}
			}
			return value{
				v: float64(stats.CPU.Usage.Total),
			}
		},
	},
	{
		name: "cpu_kernel",
		help: "The total kernel cpu time",
		unit: metrics.Nanoseconds,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.CPU == nil {
				return value{}
			}
			return value{
				v: float64(stats.CPU.Usage.Kernel),
			}
		},
	},
	{
		name: "cpu_user",
		help: "The total user cpu time",
		unit: metrics.Nanoseconds,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.CPU == nil {
				return value{}
			}
			return value{
				v: float64(stats.CPU.Usage.User),
			}
		},
	},
	{
		name: "cpu_throttle_periods",
		help: "The total cpu throttle periods",
		unit: metrics.Total,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.CPU == nil {
				return value{}
			}
			return value{
				v: float64(stats.CPU.Throttling.Periods),
			}
		},
	},
	{
		name: "container_cpu_cfs_throttled_periods_total",
		help: "Number of throttled period intervals.",
		unit: metrics.Total,
		vt:   prometheus.CounterValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.CPU == nil {
				return value{}
			}
			return value{
				v: float64(stats.CPU.Throttling.ThrottledPeriods),
			}
		},
	},
	{

		name: "cpu_throttled_time",
		help: "The total cpu throttled time",
		unit: metrics.Nanoseconds,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.CPU == nil {
				return value{}
			}
			return value{
				v: float64(stats.CPU.Throttling.ThrottledTime),
			}
		},
	},
}

var memoryMetrics = []*metric{
	{
		name: "memory_cache",
		help: "The cache amount used",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.Cache),
			}
		},
	},
	{
		name: "memory_rss",
		help: "The rss amount used",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.RSS),
			}
		},
	},
	{
		name: "memory_rss_huge",
		help: "The rss_huge amount used",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.RSSHuge),
			}
		},
	},
	{
		name: "memory_mapped_file",
		help: "The mapped_file amount used",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.MappedFile),
			}
		},
	},
	{
		name: "memory_pgmajfault",
		help: "The pgmajfault amount",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.PgMajFault),
			}
		},
	},
	{
		name: "memory_inactive_anon",
		help: "The inactive_anon amount",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.InactiveAnon),
			}
		},
	},
	{
		name: "memory_active_anon",
		help: "The active_anon amount",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.ActiveAnon),
			}
		},
	},
	{
		name: "memory_inactive_file",
		help: "The inactive_file amount",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.InactiveFile),
			}
		},
	},
	{
		name: "memory_active_file",
		help: "The active_file amount",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.ActiveFile),
			}
		},
	},
	{
		name: "memory_total_cache",
		help: "The total_cache amount used",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.TotalCache),
			}
		},
	},
	{
		name: "memory_total_rss",
		help: "The total_rss amount used",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.TotalRSS),
			}
		},
	},
	{
		name: "memory_total_mapped_file",
		help: "The total_mapped_file amount used",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.TotalMappedFile),
			}
		},
	},
	{
		name: "memory_usage_failcnt",
		help: "The usage failcnt",
		unit: metrics.Total,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.Usage.Failcnt),
			}
		},
	},
	{
		name: "memory_usage_limit",
		help: "The memory limit",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.Usage.Limit),
			}
		},
	},
	{
		name: "memory_usage_max",
		help: "The memory maximum usage",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.Usage.Max),
			}
		},
	},
	{
		name: "memory_usage_usage",
		help: "The memory usage",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.Usage.Usage),
			}
		},
	},
	{
		name: "memory_kernel_failcnt",
		help: "The kernel failcnt",
		unit: metrics.Total,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.Kernel.Failcnt),
			}
		},
	},
	{
		name: "memory_kernel_limit",
		help: "The kernel limit",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.Kernel.Limit),
			}
		},
	},
	{
		name: "memory_kernel_usage",
		help: "The kernel usage",
		unit: metrics.Bytes,
		vt:   prometheus.GaugeValue,
		getValues: func(stats *v1.Metrics) value {
			if stats.Memory == nil {
				return value{}
			}
			return value{
				v: float64(stats.Memory.Kernel.Usage),
			}
		},
	},
}
