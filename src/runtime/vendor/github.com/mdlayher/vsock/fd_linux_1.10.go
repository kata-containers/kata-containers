//+build go1.10,!go1.11,linux

package vsock

import (
	"fmt"
	"os"
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

func (lfd *sysListenFD) setNonblocking(name string) error {
	// Go 1.10 doesn't support non-blocking I/O.
	if err := unix.SetNonblock(lfd.fd, false); err != nil {
		return err
	}

	lfd.f = os.NewFile(uintptr(lfd.fd), name)

	return nil
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

func (cfd *sysConnFD) setNonblocking(name string) error {
	// Go 1.10 doesn't support non-blocking I/O.
	if err := unix.SetNonblock(cfd.fd, false); err != nil {
		return err
	}

	cfd.f = os.NewFile(uintptr(cfd.fd), name)

	return nil
}

func (cfd *sysConnFD) setDeadline(t time.Time, typ deadlineType) error {
	// Deadline functionality is not available in this version on Go.
	return fmt.Errorf("vsock: connection deadlines not supported on %s", runtime.Version())
}
