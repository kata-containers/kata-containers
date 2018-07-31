// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"io"
	"os"
	"os/signal"
	"sync"
	"syscall"

	"github.com/moby/moby/pkg/term"
	"github.com/sirupsen/logrus"
	context "golang.org/x/net/context"

	pb "github.com/kata-containers/agent/protocols/grpc"
)

const sigChanSize = 2048

var sigIgnored = map[syscall.Signal]bool{
	syscall.SIGCHLD:  true,
	syscall.SIGPIPE:  true,
	syscall.SIGWINCH: true,
}

type shim struct {
	containerID string
	execID      string

	ctx   context.Context
	agent *shimAgent
}

func newShim(addr, containerID, execID string) (*shim, error) {
	agent, err := newShimAgent(addr)
	if err != nil {
		return nil, err
	}

	return &shim{containerID: containerID,
		execID: execID,
		ctx:    context.Background(),
		agent:  agent}, nil
}

func (s *shim) proxyStdio(wg *sync.WaitGroup, terminal bool) {
	// don't wait the copying of the stdin, because `io.Copy(inPipe, os.Stdin)`
	// can't terminate when no input. todo: find a better way.
	wg.Add(1)
	if !terminal {
		// In case it's not a terminal, we also need to get the output
		// from stderr.
		wg.Add(1)
	}

	inPipe, outPipe, errPipe := shimStdioPipe(s.ctx, s.agent, s.containerID, s.execID)
	go func() {
		_, err1 := io.Copy(inPipe, os.Stdin)
		_, err2 := s.agent.CloseStdin(s.ctx, &pb.CloseStdinRequest{
			ContainerId: s.containerID,
			ExecId:      s.execID})
		if err1 != nil {
			logger().WithError(err1).Warn("copy stdin failed")
		}
		if err2 != nil {
			logger().WithError(err2).Warn("close stdin failed")
		}
	}()

	go func() {
		_, err := io.Copy(os.Stdout, outPipe)
		if err != nil {
			logger().WithError(err).Info("copy stdout failed")
		}

		wg.Done()
	}()

	if !terminal {
		go func() {
			_, err := io.Copy(os.Stderr, errPipe)
			if err != nil {
				logger().WithError(err).Info("copy stderr failed")
			}

			wg.Done()
		}()
	}
}

// handleSignals performs all signal handling.
func (s *shim) handleSignals() chan os.Signal {
	sigc := make(chan os.Signal, sigChanSize)
	// handle all signals for the process.
	signal.Notify(sigc)
	signal.Ignore(syscall.SIGCHLD, syscall.SIGPIPE)

	go func() {
		for sig := range sigc {
			sysSig, ok := sig.(syscall.Signal)
			if !ok {
				err := errors.New("unknown signal")
				logger().WithError(err).WithField("signal", sig.String()).Error()
				continue
			}

			if sigIgnored[sysSig] {
				//ignore these
				continue
			}

			if debug && nonFatalSignal(sysSig) {
				logger().WithField("signal", sig).Debug("handling signal")
				backtrace()
			}

			// forward this signal to container
			_, err := s.agent.SignalProcess(s.ctx, &pb.SignalProcessRequest{
				ContainerId: s.containerID,
				ExecId:      s.execID,
				Signal:      uint32(sysSig)})
			if err != nil {
				logger().WithError(err).WithField("signal", sig.String()).Error("forward signal failed")
			}

			if fatalSignal(sysSig) {
				logger().WithField("signal", sig).Error("received fatal signal")
				die()
			}
		}
	}()
	return sigc
}

func (s *shim) resizeTty(fromTty *os.File) error {
	fd := fromTty.Fd()

	ws, err := term.GetWinsize(fd)
	if err != nil {
		logger().WithError(err).WithField("fd", fd).Info("Error getting window size")
		return nil
	}

	_, err = s.agent.TtyWinResize(s.ctx, &pb.TtyWinResizeRequest{
		ContainerId: s.containerID,
		ExecId:      s.execID,
		Row:         uint32(ws.Height),
		Column:      uint32(ws.Width)})
	if err != nil {
		logger().WithError(err).WithFields(logrus.Fields{
			"window-height": ws.Height,
			"window-width":  ws.Width,
		}).Error("set window size failed")
	}

	return err
}

func (s *shim) monitorTtySize(tty *os.File) {
	s.resizeTty(tty)
	sigchan := make(chan os.Signal, 1)
	signal.Notify(sigchan, syscall.SIGWINCH)
	go func() {
		for range sigchan {
			s.resizeTty(tty)
		}
	}()
}

func (s *shim) wait() (int32, error) {
	resp, err := s.agent.WaitProcess(s.ctx, &pb.WaitProcessRequest{
		ContainerId: s.containerID,
		ExecId:      s.execID})
	if err != nil {
		return 0, err
	}

	return resp.Status, nil
}
