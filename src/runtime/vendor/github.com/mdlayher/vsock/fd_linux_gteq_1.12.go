//+build go1.12,linux

package vsock

import (
	"os"
	"syscall"
	"time"

	"golang.org/x/sys/unix"
)

func (lfd *sysListenFD) accept4(flags int) (int, unix.Sockaddr, error) {
	// In Go 1.12+, we make use of runtime network poller integration to allow
	// net.Listener.Accept to be unblocked by a call to net.Listener.Close.
	rc, err := lfd.f.SyscallConn()
	if err != nil {
		return 0, nil, err
	}

	var (
		newFD int
		sa    unix.Sockaddr
	)

	doErr := rc.Read(func(fd uintptr) bool {
		newFD, sa, err = unix.Accept4(int(fd), flags)

		switch err {
		case unix.EAGAIN, unix.ECONNABORTED:
			// Return false to let the poller wait for readiness. See the
			// source code for internal/poll.FD.RawRead for more details.
			//
			// When the socket is in non-blocking mode, we might see EAGAIN if
			// the socket is not ready for reading.
			//
			// In addition, the network poller's accept implementation also
			// deals with ECONNABORTED, in case a socket is closed before it is
			// pulled from our listen queue.
			return false
		default:
			// No error or some unrecognized error, treat this Read operation
			// as completed.
			return true
		}
	})
	if doErr != nil {
		return 0, nil, doErr
	}

	return newFD, sa, err
}

func (lfd *sysListenFD) setDeadline(t time.Time) error { return lfd.f.SetDeadline(t) }

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

func (cfd *sysConnFD) shutdown(how int) error {
	rc, err := cfd.f.SyscallConn()
	if err != nil {
		return err
	}

	doErr := rc.Control(func(fd uintptr) {
		err = unix.Shutdown(int(fd), how)
	})
	if doErr != nil {
		return doErr
	}

	return err
}

func (cfd *sysConnFD) syscallConn() (syscall.RawConn, error) { return cfd.f.SyscallConn() }

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
