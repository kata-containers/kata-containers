//+build linux

package vsock

import (
	"net"

	"golang.org/x/sys/unix"
)

var _ net.Listener = &listener{}

// A listener is the net.Listener implementation for connection-oriented
// VM sockets.
type listener struct {
	fd   fd
	addr *Addr
}

// Addr and Close implement the net.Listener interface for listener.
func (l *listener) Addr() net.Addr { return l.addr }
func (l *listener) Close() error   { return l.fd.Close() }

// Accept accepts a single connection from the listener, and sets up
// a net.Conn backed by conn.
func (l *listener) Accept() (net.Conn, error) {
	cfd, sa, err := l.fd.Accept4(0)
	if err != nil {
		return nil, err
	}

	savm := sa.(*unix.SockaddrVM)
	remoteAddr := &Addr{
		ContextID: savm.CID,
		Port:      savm.Port,
	}

	return &conn{
		File:       cfd.NewFile(l.addr.fileName()),
		localAddr:  l.addr,
		remoteAddr: remoteAddr,
	}, nil
}

// listenStream is the entry point for ListenStream on Linux.
func listenStream(port uint32) (net.Listener, error) {
	var cid uint32
	if err := localContextID(sysFS{}, &cid); err != nil {
		return nil, err
	}

	fd, err := unix.Socket(unix.AF_VSOCK, unix.SOCK_STREAM, 0)
	if err != nil {
		return nil, err
	}

	lfd := &sysFD{fd: fd}
	return listenStreamLinuxHandleError(lfd, cid, port)
}

// listenStreamLinuxHandleError ensures that any errors from listenStreamLinux
// result in the socket being cleaned up properly.
func listenStreamLinuxHandleError(lfd fd, cid, port uint32) (net.Listener, error) {
	l, err := listenStreamLinux(lfd, cid, port)
	if err != nil {
		// If any system calls fail during setup, the socket must be closed
		// to avoid file descriptor leaks.
		_ = lfd.Close()
		return nil, err
	}

	return l, nil
}

// TODO(mdlayher): fine-tune this number instead of just picking one.
const listenBacklog = 32

// listenStreamLinux is the entry point for tests on Linux.
func listenStreamLinux(lfd fd, cid, port uint32) (net.Listener, error) {
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

	if err := lfd.Listen(listenBacklog); err != nil {
		return nil, err
	}

	lsa, err := lfd.Getsockname()
	if err != nil {
		return nil, err
	}

	lsavm := lsa.(*unix.SockaddrVM)
	addr := &Addr{
		ContextID: lsavm.CID,
		Port:      lsavm.Port,
	}

	return &listener{
		fd:   lfd,
		addr: addr,
	}, nil
}
