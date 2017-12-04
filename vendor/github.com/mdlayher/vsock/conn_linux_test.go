//+build linux

package vsock

import (
	"errors"
	"os"
	"reflect"
	"testing"

	"golang.org/x/sys/unix"
)

func Test_dialStreamLinuxHandleError(t *testing.T) {
	var closed bool
	lfd := &testFD{
		// Track when fd.Close is called.
		close: func() error {
			closed = true
			return nil
		},
		// Always return an error on connect.
		connect: func(sa unix.Sockaddr) error {
			return errors.New("error during connect")
		},
	}

	if _, err := dialStreamLinuxHandleError(lfd, 0, 0); err == nil {
		t.Fatal("expected an error, but none occurred")
	}

	if want, got := true, closed; want != got {
		t.Fatalf("unexpected socket close value:\n- want: %v\n-  got: %v",
			want, got)
	}
}

func Test_dialStreamLinuxFull(t *testing.T) {
	const (
		localFD   uintptr = 10
		localCID  uint32  = 3
		localPort uint32  = 1024

		remoteCID  uint32 = ContextIDHost
		remotePort uint32 = 2048
	)

	lsa := &unix.SockaddrVM{
		CID:  localCID,
		Port: localPort,
	}

	rsa := &unix.SockaddrVM{
		CID:  remoteCID,
		Port: remotePort,
	}

	connectFn := func(sa unix.Sockaddr) error {
		if want, got := rsa, sa; !reflect.DeepEqual(want, got) {
			t.Fatalf("unexpected connect sockaddr:\n- want: %#v\n-  got: %#v",
				want, got)
		}

		return nil
	}

	lfd := &testFD{
		connect: connectFn,
		getsockname: func() (unix.Sockaddr, error) {
			return lsa, nil
		},
		newFile: func(name string) *os.File {
			return os.NewFile(localFD, name)
		},
	}

	nc, err := dialStreamLinux(lfd, remoteCID, remotePort)
	if err != nil {
		t.Fatalf("failed to dial: %v", err)
	}

	c := nc.(*conn)

	localAddr := &Addr{
		ContextID: localCID,
		Port:      localPort,
	}

	if want, got := localAddr, c.LocalAddr(); !reflect.DeepEqual(want, got) {
		t.Fatalf("unexpected conn local address:\n- want: %#v\n-  got: %#v",
			want, got)
	}

	remoteAddr := &Addr{
		ContextID: remoteCID,
		Port:      remotePort,
	}

	if want, got := remoteAddr, c.RemoteAddr(); !reflect.DeepEqual(want, got) {
		t.Fatalf("unexpected conn remote address:\n- want: %#v\n-  got: %#v",
			want, got)
	}

	if want, got := localFD, c.File.Fd(); want != got {
		t.Fatalf("unexpected conn file descriptor:\n- want: %d\n-  got: %d",
			want, got)
	}
}
