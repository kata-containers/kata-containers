// Copyright 2026 GRG
// SPDX-License-Identifier: Apache-2.0

package client

import (
	"errors"
	"net"
	"sync/atomic"
	"testing"
	"time"
)

func TestCommonDialerDeadlineDoesNotWaitForBlockedAttempt(t *testing.T) {
	entered, release, finished := make(chan struct{}), make(chan struct{}), make(chan struct{})
	timeoutErr := errors.New("test deadline")
	defer close(release)
	go func() {
		defer close(finished)
		_, err := commonDialer(10*time.Millisecond, func() (net.Conn, error) {
			close(entered)
			<-release
			return nil, errors.New("late failure")
		}, timeoutErr)
		if !errors.Is(err, timeoutErr) {
			t.Errorf("got %v, want caller deadline", err)
		}
	}()
	<-entered
	select {
	case <-finished:
	case <-time.After(200 * time.Millisecond):
		t.Fatal("caller deadline waited for the blocked attempt")
	}
}

type observedConn struct {
	net.Conn
	closed chan struct{}
}

func (c *observedConn) Close() error {
	close(c.closed)
	return c.Conn.Close()
}

func TestCommonDialerClosesConnectionReturnedAfterDeadline(t *testing.T) {
	left, right := net.Pipe()
	defer right.Close()
	conn := &observedConn{Conn: left, closed: make(chan struct{})}
	release, finished := make(chan struct{}), make(chan struct{})
	go func() {
		defer close(finished)
		_, _ = commonDialer(10*time.Millisecond, func() (net.Conn, error) {
			<-release
			return conn, nil
		}, errors.New("test deadline"))
	}()
	select {
	case <-finished:
	case <-time.After(200 * time.Millisecond):
		close(release)
		t.Fatal("caller did not return before the connection became available")
	}
	close(release)
	select {
	case <-conn.closed:
	case <-time.After(200 * time.Millisecond):
		t.Fatal("late connection was not closed")
	}
}

func TestCommonDialerTransfersSuccessfulConnectionOnce(t *testing.T) {
	left, right := net.Pipe()
	defer right.Close()
	defer left.Close()
	var calls atomic.Int32
	conn, err := commonDialer(time.Second, func() (net.Conn, error) {
		if calls.Add(1) < 3 {
			return nil, errors.New("not listening yet")
		}
		return left, nil
	}, errors.New("test deadline"))
	if err != nil || conn != left || calls.Load() != 3 {
		t.Fatalf("unexpected dial result: conn=%v err=%v calls=%d", conn, err, calls.Load())
	}
}
