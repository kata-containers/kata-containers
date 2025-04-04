// Copyright (c) 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// gRPC client wrapper

package client

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"net"
	"net/url"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/mdlayher/vsock"
	"github.com/sirupsen/logrus"
	"go.opentelemetry.io/otel"
	otelLabel "go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/trace"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	grpcStatus "google.golang.org/grpc/status"

	"github.com/containerd/ttrpc"
	agentgrpc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
)

const (
	VSockSocketScheme     = "vsock"
	HybridVSockScheme     = "hvsock"
	RemoteSockScheme      = "remote"
	MockHybridVSockScheme = "mock"
)

var defaultDialTimeout = 30 * time.Second

var hybridVSockPort uint32
var hybridVSockErrors uint32 = 0

const hybridVSockErrorsSkip uint32 = 128

var agentClientFields = logrus.Fields{
	"name":   "agent-client",
	"pid":    os.Getpid(),
	"source": "agent-client",
}

var agentClientLog = logrus.WithFields(agentClientFields)

// AgentClient is an agent gRPC client connection wrapper for agentgrpc.AgentServiceClient
type AgentClient struct {
	AgentServiceClient agentgrpc.AgentServiceService
	HealthClient       agentgrpc.HealthService
	conn               *ttrpc.Client
}

type dialer func(string, time.Duration) (net.Conn, error)

// NewAgentClient creates a new agent gRPC client and handles both unix and vsock addresses.
//
// Supported sock address formats are:
//   - vsock://<cid>:<port>
//   - hvsock://<path>:<port>. Firecracker implements the virtio-vsock device
//     model, and mediates communication between AF_UNIX sockets (on the host end)
//     and AF_VSOCK sockets (on the guest end).
//   - mock://<path>. just for test use.
func NewAgentClient(ctx context.Context, sock string, timeout uint32) (*AgentClient, error) {
	grpcAddr, parsedAddr, err := parse(sock)
	if err != nil {
		return nil, err
	}

	dialTimeout := defaultDialTimeout
	if timeout > 0 {
		dialTimeout = time.Duration(timeout) * time.Second
		agentClientLog.WithField("timeout", timeout).Debug("custom dialing timeout has been set")
	}

	var conn net.Conn
	var d = agentDialer(parsedAddr)
	conn, err = d(grpcAddr, dialTimeout)
	if err != nil {
		return nil, err
	}

	client := ttrpc.NewClient(conn, ttrpc.WithUnaryClientInterceptor(TraceUnaryClientInterceptor()))

	return &AgentClient{
		AgentServiceClient: agentgrpc.NewAgentServiceClient(client),
		HealthClient:       agentgrpc.NewHealthClient(client),
		conn:               client,
	}, nil
}

// Close an existing connection to the agent gRPC server.
func (c *AgentClient) Close() error {
	return c.conn.Close()
}

func TraceUnaryClientInterceptor() ttrpc.UnaryClientInterceptor {
	return func(
		ctx context.Context,
		req *ttrpc.Request,
		resp *ttrpc.Response,
		ci *ttrpc.UnaryClientInfo,
		invoker ttrpc.Invoker,
	) error {
		requestMetadata := make(ttrpc.MD)

		tracer := otel.Tracer("kata")
		var span trace.Span
		ctx, span = tracer.Start(
			ctx,
			fmt.Sprintf("ttrpc.%s", req.Method),
			trace.WithSpanKind(trace.SpanKindClient),
		)
		defer span.End()

		inject(ctx, &requestMetadata)
		ctx = ttrpc.WithMetadata(ctx, requestMetadata)
		setRequest(req, &requestMetadata)

		err := invoker(ctx, req, resp)

		if err != nil {
			span.SetAttributes(otelLabel.Key("RPC_ERROR").Bool(true))
		}
		// err can be nil, that will return an OK response code
		if status, _ := status.FromError(err); status != nil {
			span.SetAttributes(otelLabel.Key("RPC_CODE").Int((int)(status.Code())))
			span.SetAttributes(otelLabel.Key("RPC_MESSAGE").String(status.Message()))
		}

		return err
	}
}

type metadataSupplier struct {
	metadata *ttrpc.MD
}

func (s *metadataSupplier) Get(key string) string {
	values, ok := s.metadata.Get(key)
	if !ok {
		return ""
	}
	return values[0]
}

func (s *metadataSupplier) Set(key string, value string) {
	s.metadata.Set(key, value)
}

// Required to satisfy Opentelemetry TextMapCarrier interface
func (s *metadataSupplier) Keys() []string {
	keys := make([]string, 0, len(*s.metadata))
	for k := range *s.metadata {
		keys = append(keys, k)
	}
	return keys
}

func inject(ctx context.Context, metadata *ttrpc.MD) {
	otel.GetTextMapPropagator().Inject(ctx, &metadataSupplier{
		metadata: metadata,
	})

}

func setRequest(req *ttrpc.Request, md *ttrpc.MD) {
	newMD := make([]*ttrpc.KeyValue, 0)
	for _, kv := range req.Metadata {
		// not found in md, means that we can copy old kv
		// otherwise, we will use the values in md to overwrite it
		if _, found := md.Get(kv.Key); !found {
			newMD = append(newMD, kv)
		}
	}

	req.Metadata = newMD

	for k, values := range *md {
		for _, v := range values {
			req.Metadata = append(req.Metadata, &ttrpc.KeyValue{
				Key:   k,
				Value: v,
			})
		}
	}
}

// vsock scheme is self-defined to be kept from being parsed by grpc.
// Any format starting with "scheme://" will be parsed by grpc and we lose
// all address information because vsock scheme is not supported by grpc.
// Therefore we use the format vsock:<cid>:<port> for vsock address.
//
// See https://github.com/grpc/grpc/blob/master/doc/naming.md
//
// In the long term, we should patch grpc to support vsock scheme and also
// upstream the timed vsock dialer.
func parse(sock string) (string, *url.URL, error) {
	addr, err := url.Parse(sock)
	if err != nil {
		return "", nil, err
	}

	var grpcAddr string
	// validate more
	switch addr.Scheme {
	case VSockSocketScheme:
		if addr.Hostname() == "" || addr.Port() == "" || addr.Path != "" {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock scheme: %s", sock)
		}
		if _, err := strconv.ParseUint(addr.Hostname(), 10, 32); err != nil {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock cid: %s", sock)
		}
		if _, err := strconv.ParseUint(addr.Port(), 10, 32); err != nil {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock port: %s", sock)
		}
		grpcAddr = VSockSocketScheme + ":" + addr.Host
	case HybridVSockScheme:
		if addr.Path == "" {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid hybrid vsock scheme: %s", sock)
		}
		hvsocket := strings.Split(addr.Path, ":")
		if len(hvsocket) != 2 {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid hybrid vsock scheme: %s", sock)
		}
		// Save port since agent dialer not sent the port to the hybridVSock dialer
		var port uint64
		if port, err = strconv.ParseUint(hvsocket[1], 10, 32); err != nil {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid hybrid vsock port %s: %v", sock, err)
		}
		hybridVSockPort = uint32(port)
		grpcAddr = HybridVSockScheme + ":" + hvsocket[0]
	case RemoteSockScheme:
		if addr.Host != "" {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid remote sock scheme: host address must be empty: %s", sock)
		}
		grpcAddr = RemoteSockScheme + ":" + addr.Path
	// just for tests use.
	case MockHybridVSockScheme:
		if addr.Path == "" {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid mock hybrid vsock scheme: %s", sock)
		}
		// e.g. mock:/tmp/socket
		grpcAddr = MockHybridVSockScheme + ":" + addr.Path
	default:
		return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid scheme: %s", sock)
	}

	return grpcAddr, addr, nil
}

func agentDialer(addr *url.URL) dialer {
	switch addr.Scheme {
	case VSockSocketScheme:
		return VsockDialer
	case HybridVSockScheme:
		return HybridVSockDialer
	case RemoteSockScheme:
		return RemoteSockDialer
	case MockHybridVSockScheme:
		return MockHybridVSockDialer
	default:
		return nil
	}
}

func parseGrpcVsockAddr(sock string) (uint32, uint32, error) {
	sp := strings.Split(sock, ":")
	if len(sp) != 3 {
		return 0, 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock address: %s", sock)
	}
	if sp[0] != VSockSocketScheme {
		return 0, 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock URL scheme: %s", sp[0])
	}

	cid, err := strconv.ParseUint(sp[1], 10, 32)
	if err != nil {
		return 0, 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock cid: %s", sp[1])
	}
	port, err := strconv.ParseUint(sp[2], 10, 32)
	if err != nil {
		return 0, 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock port: %s", sp[2])
	}

	return uint32(cid), uint32(port), nil
}

func parseGrpcHybridVSockAddr(sock string) (string, uint32, error) {
	sp := strings.Split(sock, ":")
	// scheme and host are required
	if len(sp) < 2 {
		return "", 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid hybrid vsock address: %s", sock)
	}
	if sp[0] != HybridVSockScheme {
		return "", 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid hybrid vsock URL scheme: %s", sock)
	}

	port := uint32(0)
	// the third is the port
	if len(sp) == 3 {
		p, err := strconv.ParseUint(sp[2], 10, 32)
		if err == nil {
			port = uint32(p)
		}
	}

	return sp[1], port, nil
}

// This would bypass the grpc dialer backoff strategy and handle dial timeout
// internally. Because we do not have a large number of concurrent dialers,
// it is not reasonable to have such aggressive backoffs which would kill kata
// containers boot up speed. For more information, see
// https://github.com/grpc/grpc/blob/master/doc/connection-backoff.md
func commonDialer(timeout time.Duration, dialFunc func() (net.Conn, error), timeoutErrMsg error) (net.Conn, error) {
	t := time.NewTimer(timeout)
	cancel := make(chan bool)
	ch := make(chan net.Conn)
	go func() {
		for {
			select {
			case <-cancel:
				// canceled or channel closed
				return
			default:
			}

			conn, err := dialFunc()
			if err == nil {
				// Send conn back iff timer is not fired
				// Otherwise there might be no one left reading it
				if t.Stop() {
					ch <- conn
				} else {
					conn.Close()
				}
				return
			}
		}
	}()

	var conn net.Conn
	var ok bool
	select {
	case conn, ok = <-ch:
		if !ok {
			return nil, timeoutErrMsg
		}
	case <-t.C:
		cancel <- true
		return nil, timeoutErrMsg
	}

	return conn, nil
}

func VsockDialer(sock string, timeout time.Duration) (net.Conn, error) {
	cid, port, err := parseGrpcVsockAddr(sock)
	if err != nil {
		return nil, err
	}

	dialFunc := func() (net.Conn, error) {
		return vsock.Dial(cid, port, nil)
	}

	timeoutErr := grpcStatus.Errorf(codes.DeadlineExceeded, "timed out connecting to vsock %d:%d", cid, port)

	return commonDialer(timeout, dialFunc, timeoutErr)
}

// HybridVSockDialer dials to a hybrid virtio socket
func HybridVSockDialer(sock string, timeout time.Duration) (net.Conn, error) {
	udsPath, port, err := parseGrpcHybridVSockAddr(sock)
	if err != nil {
		return nil, err
	}

	dialFunc := func() (net.Conn, error) {
		handshakeTimeout := 10 * time.Second
		conn, err := net.DialTimeout("unix", udsPath, timeout)
		if err != nil {
			return nil, err
		}

		if port == 0 {
			// use the port read at parse()
			port = hybridVSockPort
		}

		// Once the connection is opened, the following command MUST BE sent,
		// the hypervisor needs to know the port number where the agent is listening in order to
		// create the connection
		if _, err = conn.Write([]byte(fmt.Sprintf("CONNECT %d\n", port))); err != nil {
			conn.Close()
			return nil, err
		}

		errChan := make(chan error)

		go func() {
			reader := bufio.NewReader(conn)
			response, err := reader.ReadString('\n')
			if err != nil {
				errChan <- err
				return
			}

			agentClientLog.WithField("response", response).Debug("HybridVsock trivial handshake")

			if strings.Contains(response, "OK") {
				errChan <- nil
			} else {
				errChan <- errors.New("HybridVsock trivial handshake failed with malformed response code")
			}
		}()

		select {
		case err = <-errChan:
			if err != nil {
				conn.Close()

				// With full debug logging enabled there might be around 1,500 redials in a tight loop, so
				// skip logging some of these failures to avoid flooding the log.
				errorsCount := hybridVSockErrors
				if errorsCount%hybridVSockErrorsSkip == 0 {
					agentClientLog.WithField("Error", err).WithField("count", errorsCount).Debug("HybridVsock trivial handshake failed")
				}
				hybridVSockErrors = errorsCount + 1

				return nil, err
			}
			return conn, nil
		case <-time.After(handshakeTimeout):
			// Timeout: kernel vsock implementation has a race condition, where no response is given
			// Instead of waiting forever for a response, timeout after a fair amount of time.
			// See: https://lore.kernel.org/netdev/668b0eda8823564cd604b1663dc53fbaece0cd4e.camel@intel.com/
			conn.Close()
			return nil, errors.New("timeout waiting for hybrid vsocket handshake")
		}
	}

	timeoutErr := grpcStatus.Errorf(codes.DeadlineExceeded, "timed out connecting to hybrid vsocket %s", sock)
	return commonDialer(timeout, dialFunc, timeoutErr)
}

// RemoteSockDialer dials to an agent in a remote hypervisor sandbox
func RemoteSockDialer(sock string, timeout time.Duration) (net.Conn, error) {

	s := strings.Split(sock, ":")
	if !(len(s) == 2 && s[0] == RemoteSockScheme) {
		return nil, fmt.Errorf("failed to parse remote sock: %q", sock)
	}
	socketPath := s[1]

	logrus.Printf("Dialing remote sock: %q %q", socketPath, sock)

	dialFunc := func() (net.Conn, error) {
		conn, err := net.Dial("unix", socketPath)
		if err != nil {
			logrus.Errorf("failed to dial remote sock %q: %v", socketPath, err)
			return nil, err
		}
		return conn, nil
	}

	timeoutErr := grpcStatus.Errorf(codes.DeadlineExceeded, "timed out connecting to remote sock: %s", socketPath)

	return commonDialer(timeout, dialFunc, timeoutErr)
}

// just for tests use.
func MockHybridVSockDialer(sock string, timeout time.Duration) (net.Conn, error) {
	sock = strings.TrimPrefix(sock, "mock:")

	dialFunc := func() (net.Conn, error) {
		return net.DialTimeout("unix", sock, timeout)
	}

	timeoutErr := grpcStatus.Errorf(codes.DeadlineExceeded, "timed out connecting to mock hybrid vsocket %s", sock)
	return commonDialer(timeout, dialFunc, timeoutErr)
}
