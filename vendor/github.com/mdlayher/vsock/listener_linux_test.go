package vsock

import (
	"errors"
	"os"
	"reflect"
	"testing"

	"golang.org/x/sys/unix"
)

func Test_listenStreamLinuxHandleError(t *testing.T) {
	var closed bool
	lfd := &testFD{
		// Track when fd.Close is called.
		close: func() error {
			closed = true
			return nil
		},
		// Always return an error on bind.
		bind: func(sa unix.Sockaddr) error {
			return errors.New("error during bind")
		},
	}

	if _, err := listenStreamLinuxHandleError(lfd, 0, 0); err == nil {
		t.Fatal("expected an error, but none occurred")
	}

	if want, got := true, closed; want != got {
		t.Fatalf("unexpected socket close value:\n- want: %v\n-  got: %v",
			want, got)
	}
}

func Test_listenStreamLinuxPortZero(t *testing.T) {
	const (
		cid  uint32 = ContextIDHost
		port uint32 = 0
	)

	lsa := &unix.SockaddrVM{
		CID: cid,
		// Expect 0 to be turned into "any port".
		Port: unix.VMADDR_PORT_ANY,
	}

	bindFn := func(sa unix.Sockaddr) error {
		if want, got := lsa, sa; !reflect.DeepEqual(want, got) {
			t.Fatalf("unexpected bind sockaddr:\n- want: %#v\n-  got: %#v",
				want, got)
		}

		return nil
	}

	lfd := &testFD{
		bind:        bindFn,
		listen:      func(n int) error { return nil },
		getsockname: func() (unix.Sockaddr, error) { return lsa, nil },
	}

	if _, err := listenStreamLinux(lfd, cid, port); err != nil {
		t.Fatalf("failed to listen: %v", err)
	}
}

func Test_listenStreamLinuxFull(t *testing.T) {
	const (
		cid  uint32 = ContextIDHost
		port uint32 = 1024
	)

	lsa := &unix.SockaddrVM{
		CID:  cid,
		Port: port,
	}

	bindFn := func(sa unix.Sockaddr) error {
		if want, got := lsa, sa; !reflect.DeepEqual(want, got) {
			t.Fatalf("unexpected bind sockaddr:\n- want: %#v\n-  got: %#v",
				want, got)
		}

		return nil
	}

	listenFn := func(n int) error {
		if want, got := listenBacklog, n; want != got {
			t.Fatalf("unexpected listen backlog:\n- want: %d\n-  got: %d",
				want, got)
		}

		return nil
	}

	lfd := &testFD{
		bind:   bindFn,
		listen: listenFn,
		getsockname: func() (unix.Sockaddr, error) {
			return lsa, nil
		},
	}

	nl, err := listenStreamLinux(lfd, cid, port)
	if err != nil {
		t.Fatalf("failed to listen: %v", err)
	}

	l := nl.(*listener)

	if want, got := cid, l.addr.ContextID; want != got {
		t.Fatalf("unexpected listener context ID:\n- want: %d\n-  got: %d",
			want, got)
	}
	if want, got := port, l.addr.Port; want != got {
		t.Fatalf("unexpected listener context ID:\n- want: %d\n-  got: %d",
			want, got)
	}
}

func Test_listenerAccept(t *testing.T) {
	const (
		connFD uintptr = 10

		cid  uint32 = 3
		port uint32 = 1024
	)

	accept4Fn := func(flags int) (fd, unix.Sockaddr, error) {
		if want, got := 0, flags; want != got {
			t.Fatalf("unexpected accept4 flags:\n- want: %d\n-  got: %d",
				want, got)
		}

		acceptFD := &testFD{
			newFile: func(name string) *os.File {
				return os.NewFile(connFD, name)
			},
		}

		acceptSA := &unix.SockaddrVM{
			CID:  cid,
			Port: port,
		}

		return acceptFD, acceptSA, nil
	}

	localAddr := &Addr{
		ContextID: ContextIDHost,
		Port:      port,
	}

	l := &listener{
		fd: &testFD{
			accept4: accept4Fn,
		},
		addr: localAddr,
	}

	nc, err := l.Accept()
	if err != nil {
		t.Fatalf("failed to accept: %v", err)
	}

	c := nc.(*conn)

	if want, got := localAddr, c.LocalAddr(); !reflect.DeepEqual(want, got) {
		t.Fatalf("unexpected conn local address:\n- want: %#v\n-  got: %#v",
			want, got)
	}

	remoteAddr := &Addr{
		ContextID: cid,
		Port:      port,
	}

	if want, got := remoteAddr, c.RemoteAddr(); !reflect.DeepEqual(want, got) {
		t.Fatalf("unexpected conn remote address:\n- want: %#v\n-  got: %#v",
			want, got)
	}

	if want, got := connFD, c.File.Fd(); want != got {
		t.Fatalf("unexpected conn file descriptor:\n- want: %d\n-  got: %d",
			want, got)
	}
}
