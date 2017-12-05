//+build linux

package vsock

import (
	"os"

	"golang.org/x/sys/unix"
)

var _ fd = &testFD{}

// A testFD is the test implementation of fd, with functions that can be set
// for each of its methods.
type testFD struct {
	accept4     func(flags int) (fd, unix.Sockaddr, error)
	bind        func(sa unix.Sockaddr) error
	close       func() error
	connect     func(sa unix.Sockaddr) error
	listen      func(n int) error
	newFile     func(name string) *os.File
	getsockname func() (unix.Sockaddr, error)
}

func (fd *testFD) Accept4(flags int) (fd, unix.Sockaddr, error) { return fd.accept4(flags) }
func (fd *testFD) Bind(sa unix.Sockaddr) error                  { return fd.bind(sa) }
func (fd *testFD) Close() error                                 { return fd.close() }
func (fd *testFD) Connect(sa unix.Sockaddr) error               { return fd.connect(sa) }
func (fd *testFD) Listen(n int) error                           { return fd.listen(n) }
func (fd *testFD) NewFile(name string) *os.File                 { return fd.newFile(name) }
func (fd *testFD) Getsockname() (unix.Sockaddr, error)          { return fd.getsockname() }
