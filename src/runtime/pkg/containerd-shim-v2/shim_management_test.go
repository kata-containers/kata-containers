// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
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

	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
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
