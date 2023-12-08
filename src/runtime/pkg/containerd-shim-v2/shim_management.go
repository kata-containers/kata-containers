// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"encoding/json"
	"expvar"
	"fmt"
	"io"
	"net/http"
	"net/http/pprof"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"google.golang.org/grpc/codes"

	cdshim "github.com/containerd/containerd/runtime/v2/shim"
	mutils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/prometheus/client_golang/prometheus"
	dto "github.com/prometheus/client_model/go"
	"github.com/prometheus/common/expfmt"
	"github.com/sirupsen/logrus"
)

const (
	DirectVolumePathKey   = "path"
	AgentUrl              = "/agent-url"
	DirectVolumeStatUrl   = "/direct-volume/stats"
	DirectVolumeResizeUrl = "/direct-volume/resize"
	IPTablesUrl           = "/iptables"
	PolicyUrl             = "/policy"
	IP6TablesUrl          = "/ip6tables"
	MetricsUrl            = "/metrics"
)

var (
	ifSupportAgentMetricsAPI = true
	shimMgtLog               = shimLog.WithField("subsystem", "shim-management")
)

type ResizeRequest struct {
	VolumePath string
	Size       uint64
}

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

func (s *service) serveVolumeStats(w http.ResponseWriter, r *http.Request) {
	val := r.URL.Query().Get(DirectVolumePathKey)
	if val == "" {
		msg := fmt.Sprintf("Required parameter %s not found", DirectVolumePathKey)
		shimMgtLog.Info(msg)
		w.WriteHeader(http.StatusBadRequest)
		w.Write([]byte(msg))
		return
	}

	volumePath, err := url.PathUnescape(val)
	if err != nil {
		shimMgtLog.WithError(err).Error("failed to unescape the volume stat url path")
		w.WriteHeader(http.StatusInternalServerError)
		w.Write([]byte(err.Error()))
		return
	}

	buf, err := s.sandbox.GuestVolumeStats(context.Background(), volumePath)
	if err != nil {
		shimMgtLog.WithError(err).WithField("volume-path", volumePath).Error("failed to get volume stats")
		w.WriteHeader(http.StatusInternalServerError)
		w.Write([]byte(err.Error()))
		return
	}
	w.Write(buf)
}

func (s *service) serveVolumeResize(w http.ResponseWriter, r *http.Request) {
	body, err := io.ReadAll(r.Body)
	if err != nil {
		shimMgtLog.WithError(err).Error("failed to read request body")
		w.WriteHeader(http.StatusInternalServerError)
		w.Write([]byte(err.Error()))
		return
	}
	var resizeReq ResizeRequest
	err = json.Unmarshal(body, &resizeReq)
	if err != nil {
		shimMgtLog.WithError(err).Error("failed to unmarshal the http request body")
		w.WriteHeader(http.StatusInternalServerError)
		w.Write([]byte(err.Error()))
		return
	}

	err = s.sandbox.ResizeGuestVolume(context.Background(), resizeReq.VolumePath, resizeReq.Size)
	if err != nil {
		shimMgtLog.WithError(err).WithField("volume-path", resizeReq.VolumePath).Error("failed to resize the volume")
		w.WriteHeader(http.StatusInternalServerError)
		w.Write([]byte(err.Error()))
		return
	}
	w.Write([]byte(""))
}

func (s *service) policyHandler(w http.ResponseWriter, r *http.Request) {
	logger := shimMgtLog.WithFields(logrus.Fields{"handler": "policy"})

	switch r.Method {
	case http.MethodPut:
		body, err := io.ReadAll(r.Body)
		if err != nil {
			logger.WithError(err).Error("failed to read request body")
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte(err.Error()))
			return
		}

		if err = s.sandbox.SetPolicy(context.Background(), string(body)); err != nil {
			logger.WithError(err).Error("failed to set policy")
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte(err.Error()))
		}
		w.Write([]byte(""))

	default:
		w.WriteHeader(http.StatusNotImplemented)
		return
	}
}

func (s *service) ip6TablesHandler(w http.ResponseWriter, r *http.Request) {
	s.genericIPTablesHandler(w, r, true)
}

func (s *service) ipTablesHandler(w http.ResponseWriter, r *http.Request) {
	s.genericIPTablesHandler(w, r, false)
}

func (s *service) genericIPTablesHandler(w http.ResponseWriter, r *http.Request, isIPv6 bool) {
	logger := shimMgtLog.WithFields(logrus.Fields{"handler": "iptables", "ipv6": isIPv6})

	switch r.Method {
	case http.MethodPut:
		body, err := io.ReadAll(r.Body)
		if err != nil {
			logger.WithError(err).Error("failed to read request body")
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte(err.Error()))
			return
		}

		if err = s.sandbox.SetIPTables(context.Background(), isIPv6, body); err != nil {
			logger.WithError(err).Error("failed to set IPTables")
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte(err.Error()))
		}
		w.Write([]byte(""))

	case http.MethodGet:
		buf, err := s.sandbox.GetIPTables(context.Background(), isIPv6)
		if err != nil {
			logger.WithError(err).Error("failed to get IPTables")
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte(err.Error()))
		}
		w.Write(buf)
	default:
		w.WriteHeader(http.StatusNotImplemented)
		return
	}
}

func (s *service) startManagementServer(ctx context.Context, ociSpec *specs.Spec) {
	// metrics socket will under sandbox's bundle path
	metricsAddress := ServerSocketAddress(s.id)

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
	m.Handle(MetricsUrl, http.HandlerFunc(s.serveMetrics))
	m.Handle(AgentUrl, http.HandlerFunc(s.agentURL))
	m.Handle(DirectVolumeStatUrl, http.HandlerFunc(s.serveVolumeStats))
	m.Handle(DirectVolumeResizeUrl, http.HandlerFunc(s.serveVolumeResize))
	m.Handle(IPTablesUrl, http.HandlerFunc(s.ipTablesHandler))
	m.Handle(PolicyUrl, http.HandlerFunc(s.policyHandler))
	m.Handle(IP6TablesUrl, http.HandlerFunc(s.ip6TablesHandler))
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

// GetSandboxesStoragePath returns the storage path where sandboxes info are stored
func GetSandboxesStoragePath() string {
	return "/run/vc/sbs"
}

// GetSandboxesStoragePathRust returns the storage path where sandboxes info are stored in runtime-rs
func GetSandboxesStoragePathRust() string {
	return "/run/kata"
}

// SocketPath returns the path of the socket using the given storagePath
func SocketPath(id string, storagePath string) string {
	return filepath.Join(string(filepath.Separator), storagePath, id, "shim-monitor.sock")
}

// SocketPathGo returns the path of the socket to be used with the go runtime
func SocketPathGo(id string) string {
	return SocketPath(id, GetSandboxesStoragePath())
}

// SocketPathRust returns the path of the socket to be used with the rust runtime
func SocketPathRust(id string) string {
	return SocketPath(id, GetSandboxesStoragePathRust())
}

// ServerSocketAddress returns the address of the unix domain socket the shim management endpoint
// should listen.
// NOTE: this code is only called by the go shim management implementation.
func ServerSocketAddress(id string) string {
	return fmt.Sprintf("unix://%s", SocketPathGo(id))
}

// ClientSocketAddress returns the address of the unix domain socket for communicating with the
// shim management endpoint
// NOTE: this code allows various go clients, e.g. kata-runtime or kata-monitor commands, to
// connect to the rust shim management implementation.
func ClientSocketAddress(id string) (string, error) {
	// get the go runtime uds path
	socketPath := SocketPathGo(id)
	// if the path not exist, use the rust runtime uds path instead
	if _, err := os.Stat(socketPath); err != nil {
		socketPath = SocketPathRust(id)
		if _, err := os.Stat(socketPath); err != nil {
			return "", fmt.Errorf("It fails to stat both %s and %s with error %v.", SocketPathGo(id), SocketPathRust(id), err)
		}
	}

	return fmt.Sprintf("unix://%s", socketPath), nil
}
