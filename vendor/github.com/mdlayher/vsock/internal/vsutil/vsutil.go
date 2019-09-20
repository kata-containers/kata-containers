// Package vsutil provides added functionality for package vsock-internal use.
package vsutil

import (
	"net"
	"os"
	"testing"
	"time"

	"github.com/mdlayher/vsock"
)

// Accept blocks until a single connection is accepted by the net.Listener.
//
// If timeout is non-zero, the listener will be closed after the timeout
// expires, even if no connection was accepted.
func Accept(l net.Listener, timeout time.Duration) (net.Conn, error) {
	// This function accommodates both Go1.12+ and Go1.11 functionality to allow
	// net.Listener.Accept to be canceled by net.Listener.Close.
	//
	// If a timeout is set, set up a timer to close the listener and either:
	// - Go 1.12+: unblock the call to Accept
	// - Go 1.11 : eventually halt the loop due to closed file descriptor
	//
	// For Go 1.12+, we could use vsock.Listener.SetDeadline, but this approach
	// using a timer works for Go 1.11 as well.
	cancel := func() {}
	if timeout != 0 {
		timer := time.AfterFunc(timeout, func() { _ = l.Close() })
		cancel = func() { timer.Stop() }
	}

	for {
		c, err := l.Accept()
		if err != nil {
			if nerr, ok := err.(net.Error); ok && nerr.Temporary() {
				time.Sleep(250 * time.Millisecond)
				continue
			}

			return nil, err
		}

		// Got a connection, stop the timer.
		cancel()
		return c, nil
	}
}

// IsHypervisor detects if this machine is a hypervisor by determining if
// /dev/vsock is available, and then if its context ID matches the one assigned
// to hosts.
func IsHypervisor(t *testing.T) bool {
	t.Helper()

	cid, err := vsock.ContextID()
	if err != nil {
		SkipDeviceError(t, err)

		t.Fatalf("failed to retrieve context ID: %v", err)
	}

	return cid == vsock.Host
}

// SkipDeviceError skips this test if err is related to a failure to access the
// /dev/vsock device.
func SkipDeviceError(t *testing.T, err error) {
	t.Helper()

	// Unwrap net.OpError if needed.
	// TODO(mdlayher): errors.Unwrap in Go 1.13.
	if nerr, ok := err.(*net.OpError); ok {
		err = nerr.Err
	}

	if os.IsNotExist(err) {
		t.Skipf("skipping, vsock device does not exist (try: 'modprobe vhost_vsock'): %v", err)
	}
	if os.IsPermission(err) {
		t.Skipf("skipping, permission denied (try: 'chmod 666 /dev/vsock'): %v", err)
	}
}

// SkipHostIntegration skips this test if this machine is a host and cannot
// perform a given test.
func SkipHostIntegration(t *testing.T) {
	t.Helper()

	if IsHypervisor(t) {
		t.Skip("skipping, this integration test must be run in a guest")
	}
}
