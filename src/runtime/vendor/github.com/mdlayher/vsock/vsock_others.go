//go:build !linux
// +build !linux

package vsock

import (
	"fmt"
	"net"
	"os"
	"runtime"
	"time"
)

// errUnimplemented is returned by all functions on platforms that
// cannot make use of VM sockets.
var errUnimplemented = fmt.Errorf("vsock: not implemented on %s/%s",
	runtime.GOOS, runtime.GOARCH)

func fileListener(_ *os.File) (*Listener, error)       { return nil, errUnimplemented }
func listen(_, _ uint32, _ *Config) (*Listener, error) { return nil, errUnimplemented }

type listener struct{}

func (*listener) Accept() (net.Conn, error)     { return nil, errUnimplemented }
func (*listener) Addr() net.Addr                { return nil }
func (*listener) Close() error                  { return errUnimplemented }
func (*listener) SetDeadline(_ time.Time) error { return errUnimplemented }

func dial(_, _ uint32, _ *Config) (*Conn, error) { return nil, errUnimplemented }

func contextID() (uint32, error) { return 0, errUnimplemented }

func isErrno(_ error, _ int) bool { return false }
