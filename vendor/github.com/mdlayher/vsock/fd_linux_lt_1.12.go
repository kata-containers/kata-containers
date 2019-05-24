//+build !go1.12,linux

package vsock

import (
	"fmt"
	"runtime"
	"syscall"
	"time"

	"golang.org/x/sys/unix"
)

func (lfd *sysListenFD) accept4(flags int) (int, unix.Sockaddr, error) {
	// In Go 1.11, accept on the raw file descriptor directly, because lfd.f
	// may be attached to the runtime network poller, forcing this call to block
	// even if Close is called.
	return unix.Accept4(lfd.fd, flags)
}

func (*sysListenFD) setDeadline(_ time.Time) error {
	// Listener deadlines won't work as expected in this version of Go, so
	// return an early error.
	return fmt.Errorf("vsock: listener deadlines not supported on %s", runtime.Version())
}

func (*sysConnFD) shutdown(_ int) error {
	// Shutdown functionality is not available in this version on Go.
	return fmt.Errorf("vsock: close conn read/write not supported on %s", runtime.Version())
}

func (*sysConnFD) syscallConn() (syscall.RawConn, error) {
	// SyscallConn functionality is not available in this version on Go.
	return nil, fmt.Errorf("vsock: syscall conn not supported on %s", runtime.Version())
}
