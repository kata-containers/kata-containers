//+build linux

package vsock

import (
	"golang.org/x/sys/unix"
)

// newConn creates a Conn using a connFD, immediately setting the connFD to
// non-blocking mode for use with the runtime network poller.
func newConn(cfd connFD, local, remote *Addr) (*Conn, error) {
	// Note: if any calls fail after this point, cfd.Close should be invoked
	// for cleanup because the socket is now non-blocking.
	if err := cfd.SetNonblocking(local.fileName()); err != nil {
		return nil, err
	}

	return &Conn{
		fd:     cfd,
		local:  local,
		remote: remote,
	}, nil
}

// dial is the entry point for Dial on Linux.
func dial(cid, port uint32) (*Conn, error) {
	cfd, err := newConnFD()
	if err != nil {
		return nil, err
	}

	return dialLinux(cfd, cid, port)
}

// dialLinux is the entry point for tests on Linux.
func dialLinux(cfd connFD, cid, port uint32) (c *Conn, err error) {
	defer func() {
		if err != nil {
			// If any system calls fail during setup, the socket must be closed
			// to avoid file descriptor leaks.
			_ = cfd.EarlyClose()
		}
	}()

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

	local := &Addr{
		ContextID: lsavm.CID,
		Port:      lsavm.Port,
	}

	remote := &Addr{
		ContextID: cid,
		Port:      port,
	}

	return newConn(cfd, local, remote)
}
