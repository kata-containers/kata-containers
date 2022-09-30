package main

import (
	"fmt"
	"net/http"
	"os"
	"sync"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"github.com/sirupsen/logrus"
	log "github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

var (
	monitorLog = logrus.WithField("source", "container-monitor")

	promNamespaceMonitor = "ctr_stats_gatherer"

	scrapeCount = prometheus.NewCounter(prometheus.CounterOpts{
		Namespace: promNamespaceMonitor,
		Name:      "scrape_count_total",
		Help:      "total scape count.",
	})

	scrapeDurationsHistogram = prometheus.NewHistogram(prometheus.HistogramOpts{
		Namespace: promNamespaceMonitor,
		Name:      "scrape_durations_milliseconds",
		Help:      "Time used to scrape from shims",
		Buckets:   prometheus.ExponentialBuckets(1, 2, 10),
	})
)

type Collector struct {
	namespace string
	address   string
	metrics   []*metric
}

func newCollector(address, namespace string) *Collector {
	monitorLog.WithFields(log.Fields{
		"namespace":          namespace,
		"containerd address": address,
	}).Info("creating new collector")

	c := &Collector{
		namespace: namespace,
		address:   address,
	}
	c.metrics = append(c.metrics, cpuMetrics...)
	c.metrics = append(c.metrics, memoryMetrics...)

	return c
}

func (collector *Collector) Describe(ch chan<- *prometheus.Desc) {
	for _, m := range collector.metrics {
		ch <- m.desc()
	}
}

func (collector *Collector) Collect(ch chan<- prometheus.Metric) {
	start := time.Now()

	scrapeCount.Inc()

	defer func() {
		scrapeDurationsHistogram.Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	// Get list of containers, and then from these, get the actual stats
	// that we'll want to send back
	containers, err := GetContainers(collector.address, collector.namespace)
	if err != nil {
		monitorLog.WithError(err).Error("failed to get list of containers from containerd")
		return
	}

	wg := &sync.WaitGroup{}
	for _, c := range containers {

		if c.containerName == "" || c.containerID == c.sandboxID {
			monitorLog.WithField("container id", c.containerID).Trace("skipping stats collection for pause container")

			continue
		}

		wg.Add(1)

		go func(c Container, results chan<- prometheus.Metric) {
			stats, err := GetContainerStats(collector.address, collector.namespace, c)
			if err != nil {
				monitorLog.WithFields(log.Fields{
					"sandbox":   c.sandboxID,
					"container": c.containerID,
				}).WithError(err).Info("failed to get container stats - likely an issue with non-running containers being tracked in containerd state")
			} else if stats != nil {
				for _, m := range collector.metrics {
					metric := m.getValues(stats)
					results <- prometheus.MustNewConstMetric(
						m.desc(), m.vt, metric.v, append([]string{c.containerName, c.sandboxNamespace, c.podName}, metric.l...)...)
				}
			}

			wg.Done()
		}(c, ch)
	}

	wg.Wait()
}

func initLog(level string) {
	containerMonitorLog := log.WithFields(log.Fields{
		"name": "container-monitor",
		"pid":  os.Getpid(),
	})

	// set log level, default to warn
	logLevel, err := log.ParseLevel(level)
	if err != nil {
		logLevel = log.WarnLevel
	}

	containerMonitorLog.Logger.SetLevel(logLevel)
	containerMonitorLog.Logger.Formatter = &log.TextFormatter{TimestampFormat: time.RFC3339Nano}

	monitorLog = containerMonitorLog
}

func main() {
	app := cli.NewApp()
	app.Name = "container-monitor"
	app.Flags = []cli.Flag{
		cli.StringFlag{
			Name:  "address,a",
			Value: "/run/containerd/containerd.sock",
			Usage: "path to the containerd socket",
		},
		cli.StringFlag{
			Name:  "namespace,ns",
			Value: "default",
			Usage: "the namespace to get container stats from",
		},
		cli.StringFlag{
			Name:  "server-port,p",
			Value: "8090",
			Usage: "The address the server listens on for HTTP requests.",
		},
		cli.StringFlag{
			Name:  "log-level",
			Value: "warn",
			Usage: "Logging level (trace/debug/info/warn/error/fatal/panic).",
		},
	}

	app.Action = func(context *cli.Context) error {

		initLog(context.GlobalString("log-level"))

		collector := newCollector(context.GlobalString("address"), context.GlobalString("namespace"))

		prometheus.MustRegister(collector)
		http.Handle("/stats", promhttp.Handler())

		port := context.GlobalString("server-port")
		monitorLog.Infof("starting to serve prometheus endpoint at localhost:%s/stats", port)

		return http.ListenAndServe(fmt.Sprintf(":%s", context.GlobalString("server-port")), nil)
	}

	if err := app.Run(os.Args); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
