//+build linux

package vsock

import (
	"errors"
	"net"
	"os"
	"time"

	"golang.org/x/sys/unix"
)

var (
	// errDeadlinesNotImplemented is returned by the SetDeadline family of methods
	// for conn, because access is not yet available to the runtime network poller
	// for non-standard sockets types.
	// See: https://github.com/golang/go/issues/10565.
	errDeadlinesNotImplemented = errors.New("vsock: deadlines not implemented")
)

var _ net.Conn = &conn{}

// A conn is the net.Conn implementation for VM sockets.
type conn struct {
	*os.File
	localAddr  *Addr
	remoteAddr *Addr
}

// LocalAddr and RemoteAddr implement the net.Conn interface for conn.
func (c *conn) LocalAddr() net.Addr  { return c.localAddr }
func (c *conn) RemoteAddr() net.Addr { return c.remoteAddr }

// SetDeadline functions implement the net.Conn interface for conn.
func (c *conn) SetDeadline(_ time.Time) error      { return errDeadlinesNotImplemented }
func (c *conn) SetReadDeadline(_ time.Time) error  { return errDeadlinesNotImplemented }
func (c *conn) SetWriteDeadline(_ time.Time) error { return errDeadlinesNotImplemented }

// dialStream is the entry point for DialStream on Linux.
func dialStream(cid, port uint32) (net.Conn, error) {
	fd, err := unix.Socket(unix.AF_VSOCK, unix.SOCK_STREAM, 0)
	if err != nil {
		return nil, err
	}

	cfd := &sysFD{fd: fd}
	return dialStreamLinuxHandleError(cfd, cid, port)
}

// dialStreamLinuxHandleError ensures that any errors from dialStreamLinux result
// in the socket being cleaned up properly.
func dialStreamLinuxHandleError(cfd fd, cid, port uint32) (net.Conn, error) {
	c, err := dialStreamLinux(cfd, cid, port)
	if err != nil {
		// If any system calls fail during setup, the socket must be closed
		// to avoid file descriptor leaks.
		_ = cfd.Close()
		return nil, err
	}

	return c, nil
}

// dialStreamLinux is the entry point for tests on Linux.
func dialStreamLinux(cfd fd, cid, port uint32) (net.Conn, error) {
	rsa := &unix.SockaddrVM{
		CID:  cid,
		Port: port,
	}

	if err := cfd.Connect(rsa); err != nil {
		return nil, err
	}

	lsa, err := cfd.Getsockname()
	if err != nil {
		return nil, err
	}

	lsavm := lsa.(*unix.SockaddrVM)
	localAddr := &Addr{
		ContextID: lsavm.CID,
		Port:      lsavm.Port,
	}

	remoteAddr := &Addr{
		ContextID: cid,
		Port:      port,
	}

	return &conn{
		File:       cfd.NewFile(remoteAddr.fileName()),
		localAddr:  localAddr,
		remoteAddr: remoteAddr,
	}, nil
}
