//+build linux

package vsock

import (
	"net"
	"os"
	"time"

	"golang.org/x/sys/unix"
)

var _ net.Conn = &conn{}

// A conn is the net.Conn implementation for VM sockets.
type conn struct {
	file       *os.File
	localAddr  *Addr
	remoteAddr *Addr
}

// Implement net.Conn for type conn.
func (c *conn) LocalAddr() net.Addr                { return c.localAddr }
func (c *conn) RemoteAddr() net.Addr               { return c.remoteAddr }
func (c *conn) SetDeadline(t time.Time) error      { return c.file.SetDeadline(t) }
func (c *conn) SetReadDeadline(t time.Time) error  { return c.file.SetReadDeadline(t) }
func (c *conn) SetWriteDeadline(t time.Time) error { return c.file.SetWriteDeadline(t) }
func (c *conn) Read(b []byte) (n int, err error)   { return c.file.Read(b) }
func (c *conn) Write(b []byte) (n int, err error)  { return c.file.Write(b) }
func (c *conn) Close() error                       { return c.file.Close() }

// newConn creates a conn using an fd with the specified file name, local, and
// remote addresses.
func newConn(cfd fd, file string, local, remote *Addr) (*conn, error) {
	// Enable integration with runtime network poller for timeout support
	// in Go 1.11+.
	if err := cfd.SetNonblock(true); err != nil {
		return nil, err
	}

	return &conn{
		file:       cfd.NewFile(file),
		localAddr:  local,
		remoteAddr: remote,
	}, nil
}

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

	// File name is the name of the local socket.
	return newConn(cfd, localAddr.fileName(), localAddr, remoteAddr)
}
