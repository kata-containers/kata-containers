package vsock

import (
	"os"

	"golang.org/x/sys/unix"
)

// A fd is an interface for a file descriptor, used to perform system
// calls or swap them out for tests.
type fd interface {
	Accept4(flags int) (fd, unix.Sockaddr, error)
	Bind(sa unix.Sockaddr) error
	Connect(sa unix.Sockaddr) error
	Close() error
	Getsockname() (unix.Sockaddr, error)
	Listen(n int) error
	NewFile(name string) *os.File
}

var _ fd = &sysFD{}

// sysFD is the system call implementation of fd.
type sysFD struct {
	fd int
}

func (fd *sysFD) Accept4(flags int) (fd, unix.Sockaddr, error) {
	// Returns a regular file descriptor, must be wrapped in another
	// sysFD for it to work properly.
	nfd, sa, err := unix.Accept4(fd.fd, flags)
	if err != nil {
		return nil, nil, err
	}

	return &sysFD{fd: nfd}, sa, nil
}
func (fd *sysFD) Bind(sa unix.Sockaddr) error         { return unix.Bind(fd.fd, sa) }
func (fd *sysFD) Close() error                        { return unix.Close(fd.fd) }
func (fd *sysFD) Connect(sa unix.Sockaddr) error      { return unix.Connect(fd.fd, sa) }
func (fd *sysFD) Listen(n int) error                  { return unix.Listen(fd.fd, n) }
func (fd *sysFD) NewFile(name string) *os.File        { return os.NewFile(uintptr(fd.fd), name) }
func (fd *sysFD) Getsockname() (unix.Sockaddr, error) { return unix.Getsockname(fd.fd) }
