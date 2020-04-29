//+build linux

package vsock

import (
	"net"
	"time"

	"golang.org/x/sys/unix"
)

var _ net.Listener = &listener{}

// A listener is the net.Listener implementation for connection-oriented
// VM sockets.
type listener struct {
	fd   listenFD
	addr *Addr
}

// Addr and Close implement the net.Listener interface for listener.
func (l *listener) Addr() net.Addr                { return l.addr }
func (l *listener) Close() error                  { return l.fd.Close() }
func (l *listener) SetDeadline(t time.Time) error { return l.fd.SetDeadline(t) }

// Accept accepts a single connection from the listener, and sets up
// a net.Conn backed by conn.
func (l *listener) Accept() (net.Conn, error) {
	// Mimic what internal/poll does and close on exec, but leave it up to
	// newConn to set non-blocking mode.
	// See: https://golang.org/src/internal/poll/sock_cloexec.go.
	//
	// TODO(mdlayher): acquire syscall.ForkLock.RLock here once the Go 1.11
	// code can be removed and we're fully using the runtime network poller in
	// non-blocking mode.
	cfd, sa, err := l.fd.Accept4(unix.SOCK_CLOEXEC)
	if err != nil {
		return nil, err
	}

	savm := sa.(*unix.SockaddrVM)
	remote := &Addr{
		ContextID: savm.CID,
		Port:      savm.Port,
	}

	return newConn(cfd, l.addr, remote)
}

// listen is the entry point for Listen on Linux.
func listen(cid, port uint32) (*Listener, error) {
	lfd, err := newListenFD()
	if err != nil {
		return nil, err
	}

	return listenLinux(lfd, cid, port)
}

// listenLinux is the entry point for tests on Linux.
func listenLinux(lfd listenFD, cid, port uint32) (l *Listener, err error) {
	defer func() {
		if err != nil {
			// If any system calls fail during setup, the socket must be closed
			// to avoid file descriptor leaks.
			_ = lfd.EarlyClose()
		}
	}()

	// Zero-value for "any port" is friendlier in Go than a constant.
	if port == 0 {
		port = unix.VMADDR_PORT_ANY
	}

	sa := &unix.SockaddrVM{
		CID:  cid,
		Port: port,
	}

	if err := lfd.Bind(sa); err != nil {
		return nil, err
	}

	if err := lfd.Listen(unix.SOMAXCONN); err != nil {
		return nil, err
	}

	lsa, err := lfd.Getsockname()
	if err != nil {
		return nil, err
	}

	// Done with blocking mode setup, transition to non-blocking before the
	// caller has a chance to start calling things concurrently that might make
	// the locking situation tricky.
	//
	// Note: if any calls fail after this point, lfd.Close should be invoked
	// for cleanup because the socket is now non-blocking.
	if err := lfd.SetNonblocking("vsock-listen"); err != nil {
		return nil, err
	}

	lsavm := lsa.(*unix.SockaddrVM)
	addr := &Addr{
		ContextID: lsavm.CID,
		Port:      lsavm.Port,
	}

	return &Listener{
		l: &listener{
			fd:   lfd,
			addr: addr,
		},
	}, nil
}
