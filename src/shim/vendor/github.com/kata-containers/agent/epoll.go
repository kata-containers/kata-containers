// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os"

	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const maxEvents = 2

type epoller struct {
	fd int
	// sockR and sockW are a pipe's files two ends, this pipe is
	// used to sync between the readStdio and the process exits.
	// once the process exits, it will close one end to notify
	// the readStdio that the process has exited and it should not
	// wait on the process's terminal which has been inherited
	// by it's children and hasn't exited.
	sockR   *os.File
	sockW   *os.File
	sockMap map[int32]*os.File
}

func newEpoller() (*epoller, error) {
	epollerFd, err := unix.EpollCreate1(unix.EPOLL_CLOEXEC)
	if err != nil {
		return nil, err
	}

	rSock, wSock, err := os.Pipe()
	if err != nil {
		return nil, err
	}

	ep := &epoller{
		fd:      epollerFd,
		sockW:   wSock,
		sockR:   rSock,
		sockMap: make(map[int32]*os.File),
	}

	if err = ep.add(rSock); err != nil {
		return nil, err
	}

	return ep, nil
}

func (ep *epoller) add(f *os.File) error {
	// add creates an epoll which is used to monitor the process's pty's master and
	// one end of its exit notify pipe. Those files will be registered with level-triggered
	// notification.

	event := unix.EpollEvent{
		Fd:     int32(f.Fd()),
		Events: unix.EPOLLHUP | unix.EPOLLIN | unix.EPOLLERR | unix.EPOLLRDHUP,
	}
	ep.sockMap[int32(f.Fd())] = f
	return unix.EpollCtl(ep.fd, unix.EPOLL_CTL_ADD, int(f.Fd()), &event)
}

// There will be three cases on the epoller once it run:
// a: only pty's master get an event;
// b: only the pipe get an event;
// c: both of pty and pipe have event occur;
// for case a, it means there is output in process's terminal and what needed to do is
// just read the terminal and send them out; for case b, it means the process has exited
// and there is no data in the terminal, thus just return the "EOF" to end the io;
// for case c, it means the process has exited but there is some data in the terminal which
// hasn't been send out, thus it should send those data out first and then send "EOF" last to
// end the io.
func (ep *epoller) run() (*os.File, error) {
	fd := int32(ep.sockR.Fd())
	events := make([]unix.EpollEvent, maxEvents)
	for {
		n, err := unix.EpollWait(ep.fd, events, -1)
		if err != nil {
			// EINTR: The call was interrupted by a signal handler before either
			// any of the requested events occurred or the timeout expired
			if err == unix.EINTR {
				continue
			}
			return nil, err
		}

		for i := 0; i < n; i++ {
			ev := &events[i]
			// fd has been assigned with one end of process's exited pipe by default, and
			// here to check is there any event occur on process's terminal, if "yes", it
			// should be dealt first, otherwise, it means the process has exited and there
			// is nothing left in the process's terminal needed to be read.
			if ev.Fd != fd {
				fd = ev.Fd
				break
			}
		}
		break
	}

	mf, exist := ep.sockMap[fd]
	if !exist {
		return nil, grpcStatus.Errorf(codes.NotFound, "File %d not found", fd)
	}

	return mf, nil
}
