// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"expvar"
	"fmt"
	"io"
	"net/http"
	"net/http/pprof"
	"path/filepath"
	"strconv"
	"strings"

	cdshim "github.com/containerd/containerd/runtime/v2/shim"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/prometheus/client_golang/prometheus"
	dto "github.com/prometheus/client_model/go"
	"github.com/prometheus/common/expfmt"

	"google.golang.org/grpc/codes"

	mutils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
)

var (
	ifSupportAgentMetricsAPI = true
	shimMgtLog               = shimLog.WithField("subsystem", "shim-management")
)

// agentURL returns URL for agent
func (s *service) agentURL(w http.ResponseWriter, r *http.Request) {
	url, err := s.sandbox.GetAgentURL()
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		w.Write([]byte(err.Error()))
		return
	}

	fmt.Fprint(w, url)
}

// serveMetrics handle /metrics requests
func (s *service) serveMetrics(w http.ResponseWriter, r *http.Request) {

	// update metrics from sandbox
	s.sandbox.UpdateRuntimeMetrics()

	// update metrics for shim process
	updateShimMetrics()

	// metrics gathered by shim
	mfs, err := prometheus.DefaultGatherer.Gather()
	if err != nil {
		return
	}

	// encode the metrics
	encoder := expfmt.NewEncoder(w, expfmt.FmtText)
	for _, mf := range mfs {
		encoder.Encode(mf)
	}

	// if using an old agent, only collect shim/sandbox metrics.
	if !ifSupportAgentMetricsAPI {
		return
	}

	// get metrics from agent
	// can not pass context to serveMetrics, so use background context
	agentMetrics, err := s.sandbox.GetAgentMetrics(context.Background())
	if err != nil {
		shimMgtLog.WithError(err).Error("failed GetAgentMetrics")
		if isGRPCErrorCode(codes.NotFound, err) {
			shimMgtLog.Warn("metrics API not supportted by this agent.")
			ifSupportAgentMetricsAPI = false
			return
		}
	}

	// decode and parse metrics from agent
	list := decodeAgentMetrics(agentMetrics)

	// encode the metrics to output
	for _, mf := range list {
		encoder.Encode(mf)
	}

	// collect pod overhead metrics need sleep to get the changes of cpu/memory resources usage
	// so here only trigger the collect operation, and the data will be gathered
	// next time collection request from Prometheus server
	go s.setPodOverheadMetrics(context.Background())
}

func decodeAgentMetrics(body string) []*dto.MetricFamily {
	// decode agent metrics
	reader := strings.NewReader(body)
	decoder := expfmt.NewDecoder(reader, expfmt.FmtText)
	list := make([]*dto.MetricFamily, 0)

	for {
		mf := &dto.MetricFamily{}
		if err := decoder.Decode(mf); err != nil {
			if err == io.EOF {
				break
			}
		} else {
			// metrics collected by prometheus(prefixed by go_ and process_ ) will to add a prefix to
			// to avoid an naming conflicts
			// this will only has effect for go version agent(Kata 1.x).
			// And rust agent will create metrics for processes with the prefix "process_"
			if mf.Name != nil && (strings.HasPrefix(*mf.Name, "go_") || strings.HasPrefix(*mf.Name, "process_")) {
				mf.Name = mutils.String2Pointer("kata_agent_" + *mf.Name)
			}

			list = append(list, mf)
		}
	}

	return list
}

func (s *service) startManagementServer(ctx context.Context, ociSpec *specs.Spec) {
	// metrics socket will under sandbox's bundle path
	metricsAddress := SocketAddress(s.id)

	listener, err := cdshim.NewSocket(metricsAddress)
	if err != nil {
		shimMgtLog.WithError(err).Error("failed to create listener")
		return
	}

	// write metrics address to filesystem
	if err := cdshim.WriteAddress("monitor_address", metricsAddress); err != nil {
		shimMgtLog.WithError(err).Errorf("failed to write metrics address")
		return
	}

	shimMgtLog.Info("kata management inited")

	// bind handler
	m := http.NewServeMux()
	m.Handle("/metrics", http.HandlerFunc(s.serveMetrics))
	m.Handle("/agent-url", http.HandlerFunc(s.agentURL))
	s.mountPprofHandle(m, ociSpec)

	// register shim metrics
	registerMetrics()

	// register sandbox metrics
	vc.RegisterMetrics()

	// start serve
	svr := &http.Server{Handler: m}
	svr.Serve(listener)
}

// mountPprofHandle provides a debug endpoint
func (s *service) mountPprofHandle(m *http.ServeMux, ociSpec *specs.Spec) {

	// return if not enabled
	if !s.config.EnablePprof {
		value, ok := ociSpec.Annotations[vcAnnotations.EnablePprof]
		if !ok {
			return
		}
		enabled, err := strconv.ParseBool(value)
		if err != nil || !enabled {
			return
		}
	}
	m.Handle("/debug/vars", expvar.Handler())
	m.Handle("/debug/pprof/", http.HandlerFunc(pprof.Index))
	m.Handle("/debug/pprof/cmdline", http.HandlerFunc(pprof.Cmdline))
	m.Handle("/debug/pprof/profile", http.HandlerFunc(pprof.Profile))
	m.Handle("/debug/pprof/symbol", http.HandlerFunc(pprof.Symbol))
	m.Handle("/debug/pprof/trace", http.HandlerFunc(pprof.Trace))
}

// SocketAddress returns the address of the abstract domain socket for communicating with the
// shim management endpoint
func SocketAddress(id string) string {
	return filepath.Join(string(filepath.Separator), "run", "vc", id, "shim-monitor")
}
