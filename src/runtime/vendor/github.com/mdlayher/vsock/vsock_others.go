//+build !linux

package vsock

import (
	"fmt"
	"net"
	"runtime"
	"syscall"
	"time"
)

var (
	// errUnimplemented is returned by all functions on platforms that
	// cannot make use of VM sockets.
	errUnimplemented = fmt.Errorf("vsock: not implemented on %s/%s",
		runtime.GOOS, runtime.GOARCH)
)

func listen(_, _ uint32) (*Listener, error) { return nil, errUnimplemented }

type listener struct{}

func (*listener) Accept() (net.Conn, error)     { return nil, errUnimplemented }
func (*listener) Addr() net.Addr                { return nil }
func (*listener) Close() error                  { return errUnimplemented }
func (*listener) SetDeadline(_ time.Time) error { return errUnimplemented }

func dial(_, _ uint32) (*Conn, error) { return nil, errUnimplemented }

type connFD struct{}

func (*connFD) LocalAddr() net.Addr                           { return nil }
func (*connFD) RemoteAddr() net.Addr                          { return nil }
func (*connFD) SetDeadline(_ time.Time, _ deadlineType) error { return errUnimplemented }
func (*connFD) Read(_ []byte) (int, error)                    { return 0, errUnimplemented }
func (*connFD) Write(_ []byte) (int, error)                   { return 0, errUnimplemented }
func (*connFD) Close() error                                  { return errUnimplemented }
func (*connFD) Shutdown(_ int) error                          { return errUnimplemented }
func (*connFD) SyscallConn() (syscall.RawConn, error)         { return nil, errUnimplemented }

func contextID() (uint32, error) { return 0, errUnimplemented }

func isErrno(_ error, _ int) bool { return false }
