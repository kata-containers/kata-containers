// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"io"
	"net/http"
	"path/filepath"
	"strings"

	"github.com/containerd/containerd/namespaces"
	cdshim "github.com/containerd/containerd/runtime/v2/shim"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/prometheus/client_golang/prometheus"
	dto "github.com/prometheus/client_model/go"
	"github.com/prometheus/common/expfmt"

	"github.com/sirupsen/logrus"

	"google.golang.org/grpc/codes"

	mutils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
)

var (
	ifSupportAgentMetricsAPI = true
)

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
		if err := encoder.Encode(mf); err != nil {
		}
	}

	// if using an old agent, only collect shim/sandbox metrics.
	if !ifSupportAgentMetricsAPI {
		return
	}

	// get metrics from agent
	agentMetrics, err := s.sandbox.GetAgentMetrics()
	if err != nil {
		logrus.WithError(err).Error("failed GetAgentMetrics")
		if isGRPCErrorCode(codes.NotFound, err) {
			logrus.Warn("metrics API not supportted by this agent.")
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
	go s.setPodOverheadMetrics()
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

func (s *service) startManagementServer(ctx context.Context) {
	// metrics socket will under sandbox's bundle path
	metricsAddress, err := socketAddress(ctx, s.id)
	if err != nil {
		logrus.Errorf("failed to create socket address: %s", err.Error())
		return
	}

	listener, err := cdshim.NewSocket(metricsAddress)
	if err != nil {
		logrus.Errorf("failed to create listener: %s", err.Error())
		return
	}

	// write metrics address to filesystem
	if err := cdshim.WriteAddress("monitor_address", metricsAddress); err != nil {
		logrus.Errorf("failed to write metrics address: %s", err.Error())
		return
	}

	logrus.Info("kata monitor inited")

	// bind hanlder
	http.HandleFunc("/metrics", s.serveMetrics)

	// register shim metrics
	registerMetrics()

	// register sandbox metrics
	vc.RegisterMetrics()

	// start serve
	svr := &http.Server{Handler: http.DefaultServeMux}
	svr.Serve(listener)
}

func socketAddress(ctx context.Context, id string) (string, error) {
	ns, err := namespaces.NamespaceRequired(ctx)
	if err != nil {
		return "", err
	}
	return filepath.Join(string(filepath.Separator), "containerd-shim", ns, id, "shim-monitor.sock"), nil
}
