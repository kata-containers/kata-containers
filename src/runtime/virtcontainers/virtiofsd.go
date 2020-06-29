// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/pkg/errors"
	log "github.com/sirupsen/logrus"
)

const (
	//Timeout to wait in secounds
	virtiofsdStartTimeout = 5
)

type Virtiofsd interface {
	// Start virtiofsd, return pid of virtiofsd process
	Start(context.Context) (pid int, err error)
	// Stop virtiofsd process
	Stop() error
}

// Helper function to check virtiofsd is serving
type virtiofsdWaitFunc func(runningCmd *exec.Cmd, stderr io.ReadCloser, debug bool) error

type virtiofsd struct {
	// path to virtiofsd daemon
	path string
	// socketPath where daemon will serve
	socketPath string
	// cache size for virtiofsd
	cache string
	// extraArgs list of extra args to append to virtiofsd command
	extraArgs []string
	// sourcePath path that daemon will help to share
	sourcePath string
	// debug flag
	debug bool
	// PID process ID of virtiosd process
	PID int
	// Neded by tracing
	ctx context.Context
	// wait helper function to check if virtiofsd is serving
	wait virtiofsdWaitFunc
}

// Open socket on behalf of virtiofsd
// return file descriptor to be used by virtiofsd.
func (v *virtiofsd) getSocketFD() (*os.File, error) {
	var listener *net.UnixListener

	if _, err := os.Stat(filepath.Dir(v.socketPath)); err != nil {
		return nil, errors.Errorf("Socket directory does not exist %s", filepath.Dir(v.socketPath))
	}

	listener, err := net.ListenUnix("unix", &net.UnixAddr{Name: v.socketPath, Net: "unix"})
	if err != nil {
		return nil, err
	}
	defer listener.Close()

	listener.SetUnlinkOnClose(false)

	return listener.File()
}

// Start the virtiofsd daemon
func (v *virtiofsd) Start(ctx context.Context) (int, error) {
	span, _ := v.trace("Start")
	defer span.Finish()
	pid := 0

	if err := v.valid(); err != nil {
		return pid, err
	}

	cmd := exec.Command(v.path)

	socketFD, err := v.getSocketFD()
	if err != nil {
		return 0, err
	}

	cmd.ExtraFiles = append(cmd.ExtraFiles, socketFD)

	// Extra files start from 2 (0: stdin, 1: stdout, 2: stderr)
	// Extra FDs for virtiofsd start from 3
	// Get the FD number for previous added socketFD
	socketFdNumber := 2 + uint(len(cmd.ExtraFiles))
	args, err := v.args(socketFdNumber)
	if err != nil {
		return pid, err
	}
	cmd.Args = append(cmd.Args, args...)

	v.Logger().WithField("path", v.path).Info()
	v.Logger().WithField("args", strings.Join(args, " ")).Info()

	if err = utils.StartCmd(cmd); err != nil {
		return pid, err
	}

	defer func() {
		if err != nil {
			cmd.Process.Kill()
		}
	}()

	if v.wait == nil {
		v.wait = waitVirtiofsReady
	}

	return pid, socketFD.Close()
}

func (v *virtiofsd) Stop() error {
	if err := v.kill(); err != nil {
		return nil
	}

	if v.socketPath == "" {
		return errors.New("vitiofsd socket path is empty")
	}

	err := os.Remove(v.socketPath)
	if err != nil {
		v.Logger().WithError(err).WithField("path", v.socketPath).Warn("removing virtiofsd socket failed")
	}
	return nil
}

func (v *virtiofsd) args(FdSocketNumber uint) ([]string, error) {
	if v.sourcePath == "" {
		return []string{}, errors.New("vitiofsd source path is empty")
	}

	if _, err := os.Stat(v.sourcePath); os.IsNotExist(err) {
		return nil, err
	}

	args := []string{
		// Send logs to syslog
		"--syslog",
		// foreground operation
		"-f",
		// cache mode for virtiofsd
		"-o", "cache=" + v.cache,
		// disable posix locking in daemon: bunch of basic posix locks properties are broken
		// apt-get update is broken if enabled
		"-o", "no_posix_lock",
		// shared directory tree
		"-o", "source=" + v.sourcePath,
		// fd number of vhost-user socket
		fmt.Sprintf("--fd=%v", FdSocketNumber),
	}

	if v.debug {
		args = append(args, "-o", "debug")
	}

	if len(v.extraArgs) != 0 {
		args = append(args, v.extraArgs...)
	}

	return args, nil
}

func (v *virtiofsd) valid() error {
	if v.path == "" {
		errors.New("virtiofsd path is empty")
	}

	if v.socketPath == "" {
		errors.New("Virtiofsd socket path is empty")
	}

	if v.sourcePath == "" {
		errors.New("virtiofsd source path is empty")

	}

	return nil
}

func (v *virtiofsd) Logger() *log.Entry {
	return virtLog.WithField("subsystem", "virtiofsd")
}

func (v *virtiofsd) trace(name string) (opentracing.Span, context.Context) {
	if v.ctx == nil {
		v.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(v.ctx, name)

	span.SetTag("subsystem", "virtiofds")

	return span, ctx
}

func waitVirtiofsReady(cmd *exec.Cmd, stderr io.ReadCloser, debug bool) error {
	if cmd == nil {
		return errors.New("cmd is nil")
	}

	sockReady := make(chan error, 1)
	go func() {
		scanner := bufio.NewScanner(stderr)
		var sent bool
		for scanner.Scan() {
			if debug {
				virtLog.WithField("source", "virtiofsd").Debug(scanner.Text())
			}
			if !sent && strings.Contains(scanner.Text(), "Waiting for vhost-user socket connection...") {
				sockReady <- nil
				sent = true
			}

		}
		if !sent {
			if err := scanner.Err(); err != nil {
				sockReady <- err

			} else {
				sockReady <- fmt.Errorf("virtiofsd did not announce socket connection")

			}

		}
		// Wait to release resources of virtiofsd process
		cmd.Process.Wait()
	}()

	var err error
	select {
	case err = <-sockReady:
	case <-time.After(virtiofsdStartTimeout * time.Second):
		err = fmt.Errorf("timed out waiting for vitiofsd ready mesage pid=%d", cmd.Process.Pid)
	}

	return err
}

func (v *virtiofsd) kill() (err error) {
	span, _ := v.trace("kill")
	defer span.Finish()

	if v.PID == 0 {
		return errors.New("invalid virtiofsd PID(0)")
	}

	err = syscall.Kill(v.PID, syscall.SIGKILL)
	if err != nil {
		v.PID = 0
	}

	return err
}

// virtiofsdMock  mock implementation for unit test
type virtiofsdMock struct {
}

// Start the virtiofsd daemon
func (v *virtiofsdMock) Start(ctx context.Context) (int, error) {
	return 9999999, nil
}

func (v *virtiofsdMock) Stop() error {
	return nil
}
