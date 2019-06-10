//+build !linux

package vsock

import (
	"fmt"
	"net"
	"runtime"
)

var (
	// errUnimplemented is returned by all functions on platforms that
	// cannot make use of VM sockets.
	errUnimplemented = fmt.Errorf("vsock: not implemented on %s/%s",
		runtime.GOOS, runtime.GOARCH)
)

func listenStream(_ uint32) (net.Listener, error) {
	return nil, errUnimplemented
}

func dialStream(_, _ uint32) (net.Conn, error) {
	return nil, errUnimplemented
}
