// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

const testAgentMetrics = `# HELP go_threads Number of OS threads created.
# TYPE go_threads gauge
go_threads 23
`

func newMetricsService(sandbox *vcmock.Sandbox) *service {
	return &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
	}
}

func scrapeMetrics(s *service, ctx context.Context) *httptest.ResponseRecorder {
	rr := httptest.NewRecorder()
	r := httptest.NewRequest(http.MethodGet, MetricsURL, nil).WithContext(ctx)
	s.serveMetrics(rr, r)
	return rr
}

func TestServeMetricsIncludesAgentMetrics(t *testing.T) {
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	sandbox.GetAgentMetricsFunc = func(context.Context) (string, error) {
		return testAgentMetrics, nil
	}

	rr := scrapeMetrics(newMetricsService(sandbox), context.Background())

	assert.Equal(t, http.StatusOK, rr.Code)
	assert.Contains(t, rr.Body.String(), "kata_agent_go_threads 23\n")
}

func TestServeMetricsReturnsAfterAgentError(t *testing.T) {
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	sandbox.GetAgentMetricsFunc = func(context.Context) (string, error) {
		// A response accompanying an error must never be decoded.
		return testAgentMetrics, errors.New("some error occurred")
	}

	rr := scrapeMetrics(newMetricsService(sandbox), context.Background())

	assert.Equal(t, http.StatusOK, rr.Code)
	assert.NotEmpty(t, rr.Body.String(), "shim metrics should remain available")
	assert.NotContains(t, rr.Body.String(), "kata_agent_go_threads")
}

func TestServeMetricsPropagatesRequestCancellation(t *testing.T) {
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	seenContext := make(chan context.Context, 1)
	sandbox.GetAgentMetricsFunc = func(ctx context.Context) (string, error) {
		seenContext <- ctx
		<-ctx.Done()
		return "", ctx.Err()
	}
	s := newMetricsService(sandbox)

	ctx, cancel := context.WithCancel(context.Background())
	result := make(chan *httptest.ResponseRecorder, 1)
	go func() {
		result <- scrapeMetrics(s, ctx)
	}()

	var agentContext context.Context
	select {
	case agentContext = <-seenContext:
	case <-time.After(time.Second):
		t.Fatal("agent call did not receive the HTTP request context")
	}

	cancel()

	select {
	case rr := <-result:
		assert.Equal(t, http.StatusOK, rr.Code)
		assert.NotEmpty(t, rr.Body.String(), "shim metrics should remain available")
	case <-time.After(time.Second):
		t.Fatal("metrics handler did not return after request cancellation")
	}
	assert.ErrorIs(t, agentContext.Err(), context.Canceled)
}

func TestServeMetricsDeadlineStartsCooldown(t *testing.T) {
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	var calls atomic.Int32
	sandbox.GetAgentMetricsFunc = func(ctx context.Context) (string, error) {
		calls.Add(1)
		<-ctx.Done()
		return "", ctx.Err()
	}
	s := newMetricsService(sandbox)

	ctx, cancel := context.WithDeadline(context.Background(), time.Now().Add(-time.Second))
	defer cancel()
	rr := scrapeMetrics(s, ctx)

	assert.Equal(t, http.StatusOK, rr.Code)
	assert.NotEmpty(t, rr.Body.String(), "shim metrics should remain available")
	require.EqualValues(t, 1, calls.Load())

	// The failure opens a per-sandbox cooldown, so an immediate retry does not
	// call the guest again.
	rr = scrapeMetrics(s, context.Background())
	assert.Equal(t, http.StatusOK, rr.Code)
	assert.NotEmpty(t, rr.Body.String(), "shim metrics should remain available")
	assert.EqualValues(t, 1, calls.Load())
}

func TestServeMetricsPanicDoesNotLeaveAgentMetricsInFlight(t *testing.T) {
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	var calls atomic.Int32
	sandbox.GetAgentMetricsFunc = func(context.Context) (string, error) {
		if calls.Add(1) == 1 {
			panic("agent metrics panic")
		}
		return testAgentMetrics, nil
	}
	s := newMetricsService(sandbox)

	func() {
		defer func() {
			assert.Equal(t, "agent metrics panic", recover())
		}()
		scrapeMetrics(s, context.Background())
	}()

	rr := scrapeMetrics(s, context.Background())
	assert.Equal(t, http.StatusOK, rr.Code)
	assert.Contains(t, rr.Body.String(), "kata_agent_go_threads 23\n")
	assert.EqualValues(t, 2, calls.Load())
}

func TestServeMetricsBlockedAgentDoesNotAccumulateCalls(t *testing.T) {
	sandbox := &vcmock.Sandbox{MockID: testSandboxID}
	var calls atomic.Int32
	var active atomic.Int32
	var maxActive atomic.Int32
	started := make(chan struct{}, 1)
	release := make(chan struct{})

	sandbox.GetAgentMetricsFunc = func(ctx context.Context) (string, error) {
		calls.Add(1)
		current := active.Add(1)
		defer active.Add(-1)

		for {
			previous := maxActive.Load()
			if current <= previous || maxActive.CompareAndSwap(previous, current) {
				break
			}
		}

		select {
		case started <- struct{}{}:
		default:
		}

		select {
		case <-release:
			return testAgentMetrics, nil
		case <-ctx.Done():
			return "", ctx.Err()
		}
	}
	s := newMetricsService(sandbox)

	firstResult := make(chan *httptest.ResponseRecorder, 1)
	go func() {
		firstResult <- scrapeMetrics(s, context.Background())
	}()

	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("first agent metrics call did not start")
	}

	const concurrentScrapes = 50
	results := make(chan *httptest.ResponseRecorder, concurrentScrapes)
	var scrapes sync.WaitGroup
	scrapes.Add(concurrentScrapes)
	for range concurrentScrapes {
		go func() {
			defer scrapes.Done()
			results <- scrapeMetrics(s, context.Background())
		}()
	}

	scrapesDone := make(chan struct{})
	go func() {
		scrapes.Wait()
		close(scrapesDone)
	}()

	select {
	case <-scrapesDone:
	case <-time.After(2 * time.Second):
		t.Fatal("concurrent scrapes waited behind the blocked guest call")
	}
	close(results)

	for rr := range results {
		assert.Equal(t, http.StatusOK, rr.Code)
		assert.NotEmpty(t, rr.Body.String(), "shim metrics should remain available")
		assert.NotContains(t, rr.Body.String(), "kata_agent_go_threads")
	}
	assert.EqualValues(t, 1, calls.Load())
	assert.EqualValues(t, 1, maxActive.Load())

	close(release)
	select {
	case rr := <-firstResult:
		assert.Equal(t, http.StatusOK, rr.Code)
		assert.Contains(t, rr.Body.String(), "kata_agent_go_threads 23\n")
	case <-time.After(time.Second):
		t.Fatal("first metrics scrape did not finish after releasing the agent")
	}
}

func TestServeMetricsCapabilityStateIsPerSandbox(t *testing.T) {
	unsupportedSandbox := &vcmock.Sandbox{MockID: "unsupported"}
	var unsupportedCalls atomic.Int32
	unsupportedSandbox.GetAgentMetricsFunc = func(context.Context) (string, error) {
		unsupportedCalls.Add(1)
		return "", status.Error(codes.NotFound, "GetMetrics is unavailable")
	}

	supportedSandbox := &vcmock.Sandbox{MockID: "supported"}
	var supportedCalls atomic.Int32
	supportedSandbox.GetAgentMetricsFunc = func(context.Context) (string, error) {
		supportedCalls.Add(1)
		return testAgentMetrics, nil
	}

	unsupportedService := newMetricsService(unsupportedSandbox)
	supportedService := newMetricsService(supportedSandbox)

	unsupportedResult := scrapeMetrics(unsupportedService, context.Background())
	assert.Equal(t, http.StatusOK, unsupportedResult.Code)
	assert.NotContains(t, unsupportedResult.Body.String(), "kata_agent_go_threads")

	// The old agent is remembered only by its own service.
	scrapeMetrics(unsupportedService, context.Background())
	assert.EqualValues(t, 1, unsupportedCalls.Load())

	supportedResult := scrapeMetrics(supportedService, context.Background())
	assert.Equal(t, http.StatusOK, supportedResult.Code)
	assert.Contains(t, supportedResult.Body.String(), "kata_agent_go_threads 23\n")
	assert.EqualValues(t, 1, supportedCalls.Load())
}

func TestIsAuthorizedPeer(t *testing.T) {
	type testData struct {
		peerUID    uint32
		selfEUID   uint32
		authorized bool
	}

	data := []testData{
		{peerUID: 0, selfEUID: 0, authorized: true},
		{peerUID: 1000, selfEUID: 0, authorized: false},
		{peerUID: 1000, selfEUID: 1000, authorized: true},
		{peerUID: 0, selfEUID: 1000, authorized: true},
		{peerUID: 1001, selfEUID: 1000, authorized: false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)
		assert.Equal(t, d.authorized, isAuthorizedPeer(d.peerUID, d.selfEUID), msg)
	}
}

// listenPeerCredTest listens on a Unix socket in dir and serves a handler that
// always answers "ok" through the listener returned by wrap.
func listenPeerCredTest(t *testing.T, dir string, wrap func(net.Listener) net.Listener) string {
	sockPath := filepath.Join(dir, "shim-monitor.sock")
	l, err := net.Listen("unix", sockPath)
	require.NoError(t, err)

	srv := &http.Server{Handler: http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.Write([]byte("ok"))
	})}
	go srv.Serve(wrap(l))
	t.Cleanup(func() { srv.Close() })

	return sockPath
}

// getOverSocket sends one request over a new connection to sockPath.
func getOverSocket(sockPath string) (string, error) {
	client := &http.Client{
		Timeout: 5 * time.Second,
		Transport: &http.Transport{
			DisableKeepAlives: true,
			DialContext: func(ctx context.Context, _, _ string) (net.Conn, error) {
				var d net.Dialer
				return d.DialContext(ctx, "unix", sockPath)
			},
		},
	}

	resp, err := client.Get("http://shim/")
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	return string(body), err
}

// assertClosedWithoutResponse checks that the server closes conn without
// answering a request sent over it.
func assertClosedWithoutResponse(t *testing.T, conn net.Conn) {
	defer conn.Close()
	require.NoError(t, conn.SetDeadline(time.Now().Add(5*time.Second)))

	// The write may fail if the server has already closed the connection.
	conn.Write([]byte("GET / HTTP/1.0\r\n\r\n"))
	resp, err := io.ReadAll(conn)

	assert.Empty(t, string(resp), "rejected connection must not be served")
	assert.NotErrorIs(t, err, os.ErrDeadlineExceeded, "server must close a rejected connection")
}

func TestPeerCredListenerAcceptsSameUser(t *testing.T) {
	sockPath := listenPeerCredTest(t, t.TempDir(), func(l net.Listener) net.Listener {
		return newPeerCredListener(l)
	})

	// The client runs as the same user as the server, whether that is root or
	// not, so the real policy lets it in.
	body, err := getOverSocket(sockPath)
	require.NoError(t, err)
	assert.Equal(t, "ok", body)
}

func TestPeerCredListenerRejectsAndKeepsServing(t *testing.T) {
	var allow atomic.Bool
	var seenUID atomic.Uint32
	sockPath := listenPeerCredTest(t, t.TempDir(), func(l net.Listener) net.Listener {
		return &peerCredListener{
			Listener: l,
			authorized: func(uid uint32) bool {
				seenUID.Store(uid)
				return allow.Load()
			},
		}
	})

	conn, err := net.Dial("unix", sockPath)
	require.NoError(t, err)
	assertClosedWithoutResponse(t, conn)
	assert.EqualValues(t, os.Geteuid(), seenUID.Load(), "policy must see the peer's effective UID")

	allow.Store(true)
	body, err := getOverSocket(sockPath)
	require.NoError(t, err, "server must keep serving after rejecting a connection")
	assert.Equal(t, "ok", body)
}

// dialAsUser connects to sockPath with the effective UID set to uid. Only the
// calling thread's UID is changed, through the raw syscall, so the rest of the
// test process keeps running as root.
func dialAsUser(sockPath string, uid int) (net.Conn, error) {
	type result struct {
		conn net.Conn
		err  error
	}
	ch := make(chan result, 1)

	go func() {
		// Never unlocked: the runtime discards the thread when this goroutine
		// exits, instead of reusing it with the changed UID.
		runtime.LockOSThread()

		if _, _, errno := unix.RawSyscall(unix.SYS_SETRESUID, ^uintptr(0), uintptr(uid), ^uintptr(0)); errno != 0 {
			ch <- result{err: fmt.Errorf("setresuid: %w", errno)}
			return
		}
		conn, err := net.Dial("unix", sockPath)
		ch <- result{conn: conn, err: err}
	}()

	r := <-ch
	return r.conn, r.err
}

func TestPeerCredListenerRejectsOtherUser(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	// Make the socket reachable by every user, as if its permissions had
	// drifted, so that only the credential check stands in the way.
	dir, err := os.MkdirTemp("", "kata-shim-mgmt-")
	require.NoError(t, err)
	t.Cleanup(func() { os.RemoveAll(dir) })
	require.NoError(t, os.Chmod(dir, 0755))

	sockPath := listenPeerCredTest(t, dir, func(l net.Listener) net.Listener {
		return newPeerCredListener(l)
	})
	require.NoError(t, os.Chmod(sockPath, 0777))

	const nobody = 65534
	conn, err := dialAsUser(sockPath, nobody)
	require.NoError(t, err)
	assertClosedWithoutResponse(t, conn)

	body, err := getOverSocket(sockPath)
	require.NoError(t, err, "root must still be served")
	assert.Equal(t, "ok", body)
}
