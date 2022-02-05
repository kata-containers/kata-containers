//go:build linux
// +build linux

package vsock

import (
	"github.com/mdlayher/socket"
	"golang.org/x/sys/unix"
)

// dial is the entry point for Dial on Linux.
func dial(cid, port uint32, _ *Config) (*Conn, error) {
	// TODO(mdlayher): Config default nil check and initialize. Pass options to
	// socket.Config where necessary.

	c, err := socket.Socket(unix.AF_VSOCK, unix.SOCK_STREAM, 0, "vsock", nil)
	if err != nil {
		return nil, err
	}

	sa := &unix.SockaddrVM{CID: cid, Port: port}
	rsa, err := c.Connect(sa)
	if err != nil {
		_ = c.Close()
		return nil, err
	}

	// TODO(mdlayher): getpeername(2) appears to return nil in the GitHub CI
	// environment, so in the event of a nil sockaddr, fall back to the previous
	// method of synthesizing the remote address.
	if rsa == nil {
		rsa = sa
	}

	lsa, err := c.Getsockname()
	if err != nil {
		_ = c.Close()
		return nil, err
	}

	lsavm := lsa.(*unix.SockaddrVM)
	rsavm := rsa.(*unix.SockaddrVM)

	return &Conn{
		c: c,
		local: &Addr{
			ContextID: lsavm.CID,
			Port:      lsavm.Port,
		},
		remote: &Addr{
			ContextID: rsavm.CID,
			Port:      rsavm.Port,
		},
	}, nil
}
