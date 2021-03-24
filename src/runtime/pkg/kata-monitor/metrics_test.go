// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"bytes"
	"strings"
	"testing"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/common/expfmt"
	"github.com/stretchr/testify/assert"
)

var (
	shimMetricBody = `# HELP go_threads Number of OS threads created.
# TYPE go_threads gauge
go_threads 23
# HELP process_open_fds Number of open file descriptors.
# TYPE process_open_fds gauge
process_open_fds 37
# HELP go_gc_duration_seconds A summary of the GC invocation durations.
# TYPE go_gc_duration_seconds summary
go_gc_duration_seconds{quantile="0"} 6.8986e-05
go_gc_duration_seconds{quantile="0.25"} 0.000148349
go_gc_duration_seconds{quantile="0.5"} 0.000184765
go_gc_duration_seconds{quantile="0.75"} 0.000209099
go_gc_duration_seconds{quantile="1"} 0.000507322
go_gc_duration_seconds_sum 1.353545751
go_gc_duration_seconds_count 6491
# HELP ttt Help for ttt.
# TYPE ttt gauge
ttt 999
`
)

func TestParsePrometheusMetrics(t *testing.T) {
	assert := assert.New(t)
	sandboxID := "sandboxID-abc"

	// parse metrics
	list, err := parsePrometheusMetrics(sandboxID, []byte(shimMetricBody))
	assert.Nil(err, "parsePrometheusMetrics should not return error")

	assert.Equal(4, len(list), "should return 3 metric families")

	// assert the first metric
	mf := list[0]
	assert.Equal("kata_shim_go_gc_duration_seconds", *mf.Name, "family name should be kata_shim_go_gc_duration_seconds")
	assert.Equal(1, len(mf.Metric), "metric count should be 1")
	assert.Equal("A summary of the GC invocation durations.", *mf.Help, "help should be `go_gc_duration_seconds A summary of the GC invocation durations.`")
	assert.Equal("SUMMARY", mf.Type.String(), "metric type should be summary")

	// get the metric
	m := mf.Metric[0]
	assert.Equal(1, len(m.Label), "should have only 1 labels")
	assert.Equal("sandbox_id", *m.Label[0].Name, "label name should be sandbox_id")
	assert.Equal(sandboxID, *m.Label[0].Value, "label value should be", sandboxID)

	summary := m.Summary
	assert.NotNil(summary, "summary should not be nil")
	assert.NotNil(6491, *summary.SampleCount, "summary count should be 6491")
	assert.NotNil(1.353545751, *summary.SampleSum, "summary count should be 1.353545751")

	quantiles := summary.Quantile
	assert.Equal(5, len(quantiles), "should have 5 quantiles")

	// the second
	assert.Equal(0.25, *quantiles[1].Quantile, "Quantile should be 0.25")
	assert.Equal(0.000148349, *quantiles[1].Value, "Value should be 0.000148349")

	// the last
	assert.Equal(1.0, *quantiles[4].Quantile, "Quantile should be 1")
	assert.Equal(0.000507322, *quantiles[4].Value, "Value should be 0.000507322")

	// assert the second metric
	mf = list[1]
	assert.Equal("kata_shim_go_threads", *mf.Name, "family name should be kata_shim_go_threads")
	assert.Equal("GAUGE", mf.Type.String(), "metric type should be gauge")
	assert.Equal("sandbox_id", *m.Label[0].Name, "label name should be sandbox_id")
	assert.Equal(sandboxID, *m.Label[0].Value, "label value should be", sandboxID)

	// assert the third metric
	mf = list[2]
	assert.Equal("kata_shim_process_open_fds", *mf.Name, "family name should be kata_shim_process_open_fds")
	assert.Equal("GAUGE", mf.Type.String(), "metric type should be gauge")
	assert.Equal("sandbox_id", *mf.Metric[0].Label[0].Name, "label name should be sandbox_id")
	assert.Equal(sandboxID, *mf.Metric[0].Label[0].Value, "label value should be", sandboxID)

	// assert the last metric
	mf = list[3]
	assert.Equal("ttt", *mf.Name, "family name should be ttt")
	assert.Equal("GAUGE", mf.Type.String(), "metric type should be gauge")
	assert.Equal("sandbox_id", *mf.Metric[0].Label[0].Name, "label name should be sandbox_id")
	assert.Equal(sandboxID, *mf.Metric[0].Label[0].Value, "label value should be", sandboxID)
}

func TestEncodeMetricFamily(t *testing.T) {
	assert := assert.New(t)
	prometheus.MustRegister(runningShimCount)
	prometheus.MustRegister(scrapeCount)

	runningShimCount.Add(11)
	scrapeCount.Inc()
	scrapeCount.Inc()

	mfs, _ := prometheus.DefaultGatherer.Gather()

	// create encoder
	buf := bytes.NewBufferString("")
	encoder := expfmt.NewEncoder(buf, expfmt.FmtText)

	// encode metrics to text format
	err := encodeMetricFamily(mfs, encoder)
	assert.Nil(err, "encodeMetricFamily should not return error")

	// here will be to many metrics,
	// we only check two metrics that we have set
	lines := strings.Split(buf.String(), "\n")
	for _, line := range lines {
		if strings.HasPrefix(line, "#") {
			continue
		}

		fields := strings.Split(line, " ")
		if len(fields) != 2 {
			continue
		}
		// only check kata_monitor_running_shim_count and kata_monitor_scrape_count
		if fields[0] == "kata_monitor_running_shim_count" {
			assert.Equal("11", fields[1], "kata_monitor_running_shim_count should be 11")
		} else if fields[0] == "kata_monitor_scrape_count" {
			assert.Equal("2", fields[1], "kata_monitor_scrape_count should be 2")
		}
	}
}
