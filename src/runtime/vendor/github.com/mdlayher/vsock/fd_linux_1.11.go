//+build go1.11,!go1.12,linux

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
	// From now on, we must perform non-blocking I/O, so that our
	// net.Listener.Accept method can be interrupted by closing the socket.
	if err := unix.SetNonblock(lfd.fd, true); err != nil {
		return err
	}

	// Transition from blocking mode to non-blocking mode.
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
	// From now on, we must perform non-blocking I/O, so that our deadline
	// methods work, and the connection can be interrupted by net.Conn.Close.
	if err := unix.SetNonblock(cfd.fd, true); err != nil {
		return err
	}

	// Transition from blocking mode to non-blocking mode.
	cfd.f = os.NewFile(uintptr(cfd.fd), name)

	return nil
}

func (cfd *sysConnFD) setDeadline(t time.Time, typ deadlineType) error {
	switch typ {
	case deadline:
		return cfd.f.SetDeadline(t)
	case readDeadline:
		return cfd.f.SetReadDeadline(t)
	case writeDeadline:
		return cfd.f.SetWriteDeadline(t)
	}

	panicf("vsock: sysConnFD.SetDeadline method invoked with invalid deadline type constant: %d", typ)
	return nil
}
