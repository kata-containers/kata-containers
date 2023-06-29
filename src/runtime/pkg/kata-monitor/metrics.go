// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"bytes"
	"compress/gzip"
	"io"
	"net/http"
	"sort"
	"strings"
	"sync"
	"time"

	containerdshim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	mutils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils/shimclient"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/common/expfmt"

	dto "github.com/prometheus/client_model/go"
)

const (
	promNamespaceMonitor  = "kata_monitor"
	contentTypeHeader     = "Content-Type"
	contentEncodingHeader = "Content-Encoding"
)

var (
	runningShimCount = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: promNamespaceMonitor,
		Name:      "running_shim_count",
		Help:      "Running shim count(running sandboxes).",
	})

	scrapeCount = prometheus.NewCounter(prometheus.CounterOpts{
		Namespace: promNamespaceMonitor,
		Name:      "scrape_count",
		Help:      "Scape count.",
	})

	scrapeFailedCount = prometheus.NewCounter(prometheus.CounterOpts{
		Namespace: promNamespaceMonitor,
		Name:      "scrape_failed_count",
		Help:      "Failed scape count.",
	})

	scrapeDurationsHistogram = prometheus.NewHistogram(prometheus.HistogramOpts{
		Namespace: promNamespaceMonitor,
		Name:      "scrape_durations_histogram_milliseconds",
		Help:      "Time used to scrape from shims",
		Buckets:   prometheus.ExponentialBuckets(1, 2, 10),
	})

	gzipPool = sync.Pool{
		New: func() interface{} {
			return gzip.NewWriter(nil)
		},
	}
)

func registerMetrics() {
	prometheus.MustRegister(runningShimCount)
	prometheus.MustRegister(scrapeCount)
	prometheus.MustRegister(scrapeFailedCount)
	prometheus.MustRegister(scrapeDurationsHistogram)
}

// ProcessMetricsRequest get metrics from shim/hypervisor/vm/agent and return metrics to client.
func (km *KataMonitor) ProcessMetricsRequest(w http.ResponseWriter, r *http.Request) {
	start := time.Now()

	scrapeCount.Inc()
	defer func() {
		scrapeDurationsHistogram.Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	// this is likely the same as `kata-runtime metrics <SANDBOX>`.
	sandboxID, err := getSandboxIDFromReq(r)
	if err == nil && sandboxID != "" {
		metrics, err := GetSandboxMetrics(sandboxID)
		if err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte(err.Error()))
			return
		}
		w.Write([]byte(metrics))
		return
	}

	// if no sandbox provided, will get all sandbox's metrics.

	// prepare writer for writing response.
	contentType := expfmt.Negotiate(r.Header)

	// set response header
	header := w.Header()
	header.Set(contentTypeHeader, string(contentType))

	// create writer
	writer := io.Writer(w)
	if mutils.GzipAccepted(r.Header) {
		header.Set(contentEncodingHeader, "gzip")
		gz := gzipPool.Get().(*gzip.Writer)
		defer gzipPool.Put(gz)

		gz.Reset(w)
		defer gz.Close()

		writer = gz
	}

	filterFamilies, err := getFilterFamilyFromReq(r)
	if err != nil {
		return
	}

	// create encoder to encode metrics.
	encoder := expfmt.NewEncoder(writer, contentType)

	if len(filterFamilies) == 0 {
		// gather metrics collected for management agent.
		mfs, err := prometheus.DefaultGatherer.Gather()
		if err != nil {
			monitorLog.WithError(err).Error("failed to Gather metrics from prometheus.DefaultGatherer")
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte(err.Error()))
			return
		}

		// encode metric gathered in current process
		if err := encodeMetricFamily(mfs, encoder); err != nil {
			monitorLog.WithError(err).Warnf("failed to encode metrics")
		}
	}

	// aggregate sandboxes metrics and write to response by encoder
	if err := km.aggregateSandboxMetrics(encoder, filterFamilies); err != nil {
		monitorLog.WithError(err).Errorf("failed aggregateSandboxMetrics")
		scrapeFailedCount.Inc()
	}
}

func encodeMetricFamily(mfs []*dto.MetricFamily, encoder expfmt.Encoder) error {
	for i := range mfs {
		metricFamily := mfs[i]

		if metricFamily.Name != nil && !strings.HasPrefix(*metricFamily.Name, promNamespaceMonitor) {
			metricFamily.Name = mutils.String2Pointer(promNamespaceMonitor + "_" + *metricFamily.Name)
		}

		// encode and write to output
		if err := encoder.Encode(metricFamily); err != nil {
			return err
		}
	}
	return nil
}

// aggregateSandboxMetrics will get metrics from one sandbox and do some process
func (km *KataMonitor) aggregateSandboxMetrics(encoder expfmt.Encoder, filterFamilies []string) error {
	// get all kata sandboxes from cache
	sandboxes := km.sandboxCache.getSandboxList()
	// save running kata pods as a metrics.
	runningShimCount.Set(float64(len(sandboxes)))

	if len(sandboxes) == 0 {
		return nil
	}

	// sandboxMetricsList contains list of MetricFamily list from one sandbox.
	sandboxMetricsList := make([][]*dto.MetricFamily, 0)

	wg := &sync.WaitGroup{}
	// used to receive response
	results := make(chan []*dto.MetricFamily, len(sandboxes))

	monitorLog.WithField("sandboxes count", len(sandboxes)).Debugf("aggregate sandbox metrics")

	// get metrics from sandbox's shim
	for _, sandboxID := range sandboxes {
		sandboxMetadata, ok := km.sandboxCache.getCRIMetadata(sandboxID)
		if !ok { // likely the sandbox has been just removed
			continue
		}
		wg.Add(1)
		go func(sandboxID string, sandboxMetadata sandboxCRIMetadata, results chan<- []*dto.MetricFamily) {
			sandboxMetrics, err := getParsedMetrics(sandboxID, sandboxMetadata)
			if err != nil {
				monitorLog.WithError(err).WithField("sandbox_id", sandboxID).Errorf("failed to get metrics for sandbox")
			}

			results <- sandboxMetrics
			wg.Done()
			monitorLog.WithField("sandbox_id", sandboxID).Debug("job finished")
		}(sandboxID, sandboxMetadata, results)

		monitorLog.WithField("sandbox_id", sandboxID).Debug("job started")
	}

	wg.Wait()
	monitorLog.Debug("all job finished")
	close(results)

	// get all job result from chan
	for sandboxMetrics := range results {
		if sandboxMetrics != nil {
			sandboxMetricsList = append(sandboxMetricsList, sandboxMetrics)
		}
	}

	if len(sandboxMetricsList) == 0 {
		return nil
	}

	// metricsMap used to aggregate metrics from multiple sandboxes
	// key is MetricFamily.Name, and value is list of MetricFamily from multiple sandboxes
	metricsMap := make(map[string]*dto.MetricFamily)
	// merge MetricFamily list for the same MetricFamily.Name from multiple sandboxes.
	for i := range sandboxMetricsList {
		sandboxMetrics := sandboxMetricsList[i]
		for j := range sandboxMetrics {
			mf := sandboxMetrics[j]
			key := *mf.Name

			// add MetricFamily.Metric to the exists MetricFamily instance
			if oldmf, found := metricsMap[key]; found {
				oldmf.Metric = append(oldmf.Metric, mf.Metric...)
			} else {
				metricsMap[key] = mf
			}
		}
	}

	// write metrics to response.
	if len(filterFamilies) > 0 {
		for _, filterName := range filterFamilies {
			for fullName, mf := range metricsMap {
				if strings.HasPrefix(fullName, filterName) {
					if err := encoder.Encode(mf); err != nil {
						return err
					}
				}
			}
		}
	} else {
		for _, mf := range metricsMap {
			if err := encoder.Encode(mf); err != nil {
				return err
			}
		}
	}
	return nil

}

func getParsedMetrics(sandboxID string, sandboxMetadata sandboxCRIMetadata) ([]*dto.MetricFamily, error) {
	body, err := shimclient.DoGet(sandboxID, defaultTimeout, containerdshim.MetricsUrl)
	if err != nil {
		return nil, err
	}

	return parsePrometheusMetrics(sandboxID, sandboxMetadata, body)
}

// GetSandboxMetrics will get sandbox's metrics from shim
func GetSandboxMetrics(sandboxID string) (string, error) {
	body, err := shimclient.DoGet(sandboxID, defaultTimeout, containerdshim.MetricsUrl)
	if err != nil {
		return "", err
	}

	return string(body), nil
}

// parsePrometheusMetrics will decode metrics from Prometheus text format
// and return array of *dto.MetricFamily with an ASC order
func parsePrometheusMetrics(sandboxID string, sandboxMetadata sandboxCRIMetadata, body []byte) ([]*dto.MetricFamily, error) {
	reader := bytes.NewReader(body)
	decoder := expfmt.NewDecoder(reader, expfmt.FmtText)

	// decode metrics from sandbox to MetricFamily
	list := make([]*dto.MetricFamily, 0)
	for {
		mf := &dto.MetricFamily{}
		if err := decoder.Decode(mf); err != nil {
			if err == io.EOF {
				break
			}
			return nil, err
		}

		metricList := mf.Metric
		for j := range metricList {
			metric := metricList[j]
			metric.Label = append(metric.Label,
				&dto.LabelPair{
					Name:  mutils.String2Pointer("sandbox_id"),
					Value: mutils.String2Pointer(sandboxID),
				},
				&dto.LabelPair{
					Name:  mutils.String2Pointer("cri_uid"),
					Value: mutils.String2Pointer(sandboxMetadata.uid),
				},
				&dto.LabelPair{
					Name:  mutils.String2Pointer("cri_name"),
					Value: mutils.String2Pointer(sandboxMetadata.name),
				},
				&dto.LabelPair{
					Name:  mutils.String2Pointer("cri_namespace"),
					Value: mutils.String2Pointer(sandboxMetadata.namespace),
				},
			)
		}

		// Kata shim are using prometheus go client, add a prefix for metric name to avoid confusing
		if mf.Name != nil && (strings.HasPrefix(*mf.Name, "go_") || strings.HasPrefix(*mf.Name, "process_")) {
			mf.Name = mutils.String2Pointer("kata_shim_" + *mf.Name)
		}

		list = append(list, mf)
	}

	// sort ASC
	sort.SliceStable(list, func(i, j int) bool {
		b := strings.Compare(*list[i].Name, *list[j].Name)
		return b < 0
	})

	return list, nil
}
