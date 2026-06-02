//go:build linux

// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"io"
	"net"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/mdlayher/vsock"
)

const (
	vsockUDSForwardScheme    = "vsock"
	vsockUDSForwardDialRetry = 2 * time.Second
)

func guestCIDFromAgentURL(agentURL string) (uint32, error) {
	const prefix = vsockUDSForwardScheme + "://"
	if !strings.HasPrefix(agentURL, prefix) {
		return 0, fmt.Errorf("vsock UDS forward requires %s agent URL, got %q", vsockUDSForwardScheme, agentURL)
	}

	rest := strings.TrimPrefix(agentURL, prefix)
	idx := strings.LastIndex(rest, ":")
	if idx <= 0 {
		return 0, fmt.Errorf("cannot parse guest CID from agent URL %q", agentURL)
	}

	cid, err := strconv.ParseUint(rest[:idx], 10, 32)
	if err != nil {
		return 0, fmt.Errorf("invalid guest CID in agent URL %q: %w", agentURL, err)
	}

	return uint32(cid), nil
}

func (s *service) trackVsockUDSConn(c net.Conn) {
	s.vsockUDSConns.Store(c, struct{}{})
}

func (s *service) untrackVsockUDSConn(c net.Conn) {
	s.vsockUDSConns.Delete(c)
}

func (s *service) closeAllVsockUDSConns() {
	s.vsockUDSConns.Range(func(key, _ interface{}) bool {
		if c, ok := key.(net.Conn); ok {
			_ = c.Close()
		}
		return true
	})
}

func (s *service) startVsockUDSForward(guestCID, port uint32, uds string) {
	if s.vsockUDSCancel != nil {
		return
	}

	ctx, cancel := context.WithCancel(s.ctx)
	s.vsockUDSCancel = cancel

	shimLog.WithFields(map[string]interface{}{
		"guest_cid": guestCID,
		"port":      port,
		"uds":       uds,
	}).Info("vsock UDS forward: started")

	s.vsockUDSForwardWg.Add(1)
	go func() {
		defer s.vsockUDSForwardWg.Done()
		s.runVsockUDSDialLoop(ctx, guestCID, port, uds)
	}()
}

func (s *service) runVsockUDSDialLoop(ctx context.Context, guestCID, port uint32, uds string) {
	logFields := map[string]interface{}{
		"guest_cid": guestCID,
		"port":      port,
		"uds":       uds,
	}

	for {
		if ctx.Err() != nil {
			return
		}

		conn, err := vsock.Dial(guestCID, port, nil)
		if err != nil {
			shimLog.WithError(err).WithFields(logFields).Debug("vsock UDS forward: guest vsock dial failed, will retry")
			if !sleepOrDone(ctx, vsockUDSForwardDialRetry) {
				return
			}
			continue
		}

		s.trackVsockUDSConn(conn)
		s.bridgeVsockUDS(conn, uds)
		s.untrackVsockUDSConn(conn)
		_ = conn.Close()

		if ctx.Err() != nil {
			return
		}

		if !sleepOrDone(ctx, vsockUDSForwardDialRetry) {
			return
		}
	}
}

func sleepOrDone(ctx context.Context, d time.Duration) bool {
	t := time.NewTimer(d)
	defer t.Stop()

	select {
	case <-ctx.Done():
		return false
	case <-t.C:
		return true
	}
}

func (s *service) bridgeVsockUDS(vconn net.Conn, uds string) {
	first := make([]byte, 1)
	n, err := vconn.Read(first)
	if err != nil {
		shimLog.WithError(err).WithField("uds", uds).Debug("vsock UDS forward: guest vsock closed before first byte")
		return
	}
	if n == 0 {
		return
	}

	uconn, err := net.Dial("unix", uds)
	if err != nil {
		shimLog.WithError(err).WithField("uds", uds).Warn("vsock UDS forward: unix dial failed")
		return
	}

	if _, err := uconn.Write(first[:n]); err != nil {
		shimLog.WithError(err).WithField("uds", uds).Warn("vsock UDS forward: failed to write first byte to unix socket")
		_ = uconn.Close()
		return
	}

	// When either leg finishes (guest vsock EOF or host UDS close), tear down both
	// sides so the bridge returns and the dial loop can accept the next guest session.
	var once sync.Once
	teardown := func() {
		once.Do(func() {
			_ = uconn.Close()
			_ = vconn.Close()
		})
	}

	var wg sync.WaitGroup
	wg.Add(2)
	go func() {
		defer wg.Done()
		_, _ = io.Copy(uconn, vconn)
		teardown()
	}()
	go func() {
		defer wg.Done()
		_, _ = io.Copy(vconn, uconn)
		teardown()
	}()
	wg.Wait()
}

func (s *service) stopVsockUDSForward() {
	if s.vsockUDSCancel == nil {
		return
	}

	s.vsockUDSCancel()
	s.vsockUDSCancel = nil
	s.closeAllVsockUDSConns()
	s.vsockUDSForwardWg.Wait()
}

func (s *service) tryStartVsockUDSForwardFromConfig() {
	if s.config == nil || s.config.VsockUDSForwardPort == 0 {
		return
	}

	if len(s.config.VsockUDSForward) > 1 {
		shimLog.WithField("ignored_entries", s.config.VsockUDSForward[1:]).Warn(
			"vsock UDS forward: only one port/socket pair is supported; ignoring additional entries",
		)
	}

	agentURL, err := s.sandbox.GetAgentURL()
	if err != nil {
		shimLog.WithError(err).Warn("vsock UDS forward: cannot get agent URL")
		return
	}

	guestCID, err := guestCIDFromAgentURL(agentURL)
	if err != nil {
		shimLog.WithError(err).WithField("agent_url", agentURL).Warn("vsock UDS forward: cannot determine guest CID")
		return
	}

	s.startVsockUDSForward(guestCID, s.config.VsockUDSForwardPort, s.config.VsockUDSForwardUDS)
}
