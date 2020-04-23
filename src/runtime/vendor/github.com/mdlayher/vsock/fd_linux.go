package vsock

import (
	"fmt"
	"io"
	"os"
	"syscall"
	"time"

	"golang.org/x/sys/unix"
)

// contextID retrieves the local context ID for this system.
func contextID() (uint32, error) {
	f, err := os.Open(devVsock)
	if err != nil {
		return 0, err
	}
	defer f.Close()

	return unix.IoctlGetUint32(int(f.Fd()), unix.IOCTL_VM_SOCKETS_GET_LOCAL_CID)
}

// A listenFD is a type that wraps a file descriptor used to implement
// net.Listener.
type listenFD interface {
	io.Closer
	EarlyClose() error
	Accept4(flags int) (connFD, unix.Sockaddr, error)
	Bind(sa unix.Sockaddr) error
	Listen(n int) error
	Getsockname() (unix.Sockaddr, error)
	SetNonblocking(name string) error
	SetDeadline(t time.Time) error
}

var _ listenFD = &sysListenFD{}

// A sysListenFD is the system call implementation of listenFD.
type sysListenFD struct {
	// These fields should never be non-zero at the same time.
	fd int      // Used in blocking mode.
	f  *os.File // Used in non-blocking mode.
}

// newListenFD creates a sysListenFD in its default blocking mode.
func newListenFD() (*sysListenFD, error) {
	fd, err := socket()
	if err != nil {
		return nil, err
	}

	return &sysListenFD{
		fd: fd,
	}, nil
}

// Blocking mode methods.

func (lfd *sysListenFD) Bind(sa unix.Sockaddr) error         { return unix.Bind(lfd.fd, sa) }
func (lfd *sysListenFD) Getsockname() (unix.Sockaddr, error) { return unix.Getsockname(lfd.fd) }
func (lfd *sysListenFD) Listen(n int) error                  { return unix.Listen(lfd.fd, n) }

func (lfd *sysListenFD) SetNonblocking(name string) error {
	return lfd.setNonblocking(name)
}

// EarlyClose is a blocking version of Close, only used for cleanup before
// entering non-blocking mode.
func (lfd *sysListenFD) EarlyClose() error { return unix.Close(lfd.fd) }

// Non-blocking mode methods.

func (lfd *sysListenFD) Accept4(flags int) (connFD, unix.Sockaddr, error) {
	// Invoke Go version-specific logic for accept.
	newFD, sa, err := lfd.accept4(flags)
	if err != nil {
		return nil, nil, err
	}

	// Create a non-blocking connFD which will be used to implement net.Conn.
	cfd := &sysConnFD{fd: newFD}
	return cfd, sa, nil
}

func (lfd *sysListenFD) Close() error {
	// In Go 1.12+, *os.File.Close will also close the runtime network poller
	// file descriptor, so that net.Listener.Accept can stop blocking.
	return lfd.f.Close()
}

func (lfd *sysListenFD) SetDeadline(t time.Time) error {
	// Invoke Go version-specific logic for setDeadline.
	return lfd.setDeadline(t)
}

// A connFD is a type that wraps a file descriptor used to implement net.Conn.
type connFD interface {
	io.ReadWriteCloser
	EarlyClose() error
	Connect(sa unix.Sockaddr) error
	Getsockname() (unix.Sockaddr, error)
	Shutdown(how int) error
	SetNonblocking(name string) error
	SetDeadline(t time.Time, typ deadlineType) error
	SyscallConn() (syscall.RawConn, error)
}

var _ connFD = &sysConnFD{}

// newConnFD creates a sysConnFD in its default blocking mode.
func newConnFD() (*sysConnFD, error) {
	fd, err := socket()
	if err != nil {
		return nil, err
	}

	return &sysConnFD{
		fd: fd,
	}, nil
}

// A sysConnFD is the system call implementation of connFD.
type sysConnFD struct {
	// These fields should never be non-zero at the same time.
	fd int      // Used in blocking mode.
	f  *os.File // Used in non-blocking mode.
}

// Blocking mode methods.

func (cfd *sysConnFD) Connect(sa unix.Sockaddr) error      { return unix.Connect(cfd.fd, sa) }
func (cfd *sysConnFD) Getsockname() (unix.Sockaddr, error) { return unix.Getsockname(cfd.fd) }

// EarlyClose is a blocking version of Close, only used for cleanup before
// entering non-blocking mode.
func (cfd *sysConnFD) EarlyClose() error { return unix.Close(cfd.fd) }

func (cfd *sysConnFD) SetNonblocking(name string) error {
	return cfd.setNonblocking(name)
}

// Non-blocking mode methods.

func (cfd *sysConnFD) Close() error {
	// *os.File.Close will also close the runtime network poller file descriptor,
	// so that read/write can stop blocking.
	return cfd.f.Close()
}

func (cfd *sysConnFD) Read(b []byte) (int, error)  { return cfd.f.Read(b) }
func (cfd *sysConnFD) Write(b []byte) (int, error) { return cfd.f.Write(b) }

func (cfd *sysConnFD) Shutdown(how int) error {
	switch how {
	case unix.SHUT_RD, unix.SHUT_WR:
		return cfd.shutdown(how)
	default:
		panicf("vsock: sysConnFD.Shutdown method invoked with invalid how constant: %d", how)
		return nil
	}
}

func (cfd *sysConnFD) SetDeadline(t time.Time, typ deadlineType) error {
	return cfd.setDeadline(t, typ)
}

func (cfd *sysConnFD) SyscallConn() (syscall.RawConn, error) { return cfd.syscallConn() }

// socket invokes unix.Socket with the correct arguments to produce a vsock
// file descriptor.
func socket() (int, error) {
	// "Mirror what the standard library does when creating file
	// descriptors: avoid racing a fork/exec with the creation
	// of new file descriptors, so that child processes do not
	// inherit [socket] file descriptors unexpectedly.
	//
	// On Linux, SOCK_CLOEXEC was introduced in 2.6.27. OTOH,
	// Go supports Linux 2.6.23 and above. If we get EINVAL on
	// the first try, it may be that we are running on a kernel
	// older than 2.6.27. In that case, take syscall.ForkLock
	// and try again without SOCK_CLOEXEC.
	//
	// For a more thorough explanation, see similar work in the
	// Go tree: func sysSocket in net/sock_cloexec.go, as well
	// as the detailed comment in syscall/exec_unix.go."
	//
	// Explanation copied from netlink, courtesy of acln:
	// https://github.com/mdlayher/netlink/pull/138.
	fd, err := unix.Socket(unix.AF_VSOCK, unix.SOCK_STREAM|unix.SOCK_CLOEXEC, 0)
	switch err {
	case nil:
		return fd, nil
	case unix.EINVAL:
		syscall.ForkLock.RLock()
		defer syscall.ForkLock.RUnlock()

		fd, err = unix.Socket(unix.AF_VSOCK, unix.SOCK_STREAM, 0)
		if err != nil {
			return 0, err
		}
		unix.CloseOnExec(fd)

		return fd, nil
	default:
		return 0, err
	}
}

// isErrno determines if an error a matches UNIX error number.
func isErrno(err error, errno int) bool {
	switch errno {
	case ebadf:
		return err == unix.EBADF
	case enotconn:
		return err == unix.ENOTCONN
	default:
		panicf("vsock: isErrno called with unhandled error number parameter: %d", errno)
		return false
	}
}

func panicf(format string, a ...interface{}) {
	panic(fmt.Sprintf(format, a...))
}
