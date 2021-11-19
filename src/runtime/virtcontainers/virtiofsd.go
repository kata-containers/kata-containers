// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/pkg/errors"
	log "github.com/sirupsen/logrus"
)

// virtiofsdTracingTags defines tags for the trace span
var virtiofsdTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "virtiofsd",
}

var (
	errVirtiofsdDaemonPathEmpty    = errors.New("virtiofsd daemon path is empty")
	errVirtiofsdSocketPathEmpty    = errors.New("virtiofsd socket path is empty")
	errVirtiofsdSourcePathEmpty    = errors.New("virtiofsd source path is empty")
	errVirtiofsdSourceNotAvailable = errors.New("virtiofsd source path not available")
)

type Virtiofsd interface {
	// Start virtiofsd, return pid of virtiofsd process
	Start(context.Context, onQuitFunc) (pid int, err error)
	// Stop virtiofsd process
	Stop(context.Context) error
}

// Helper function to execute when virtiofsd quit
type onQuitFunc func()

type virtiofsd struct {
	// Neded by tracing
	ctx context.Context
	// path to virtiofsd daemon
	path string
	// socketPath where daemon will serve
	socketPath string
	// cache size for virtiofsd
	cache string
	// sourcePath path that daemon will help to share
	sourcePath string
	// extraArgs list of extra args to append to virtiofsd command
	extraArgs []string
	// debug flag
	debug bool
	// PID process ID of virtiosd process
	PID int
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

	// Need to change the filesystem ownership of the socket because virtiofsd runs as root while qemu can run as non-root.
	// This can be removed once virtiofsd can also run as non-root (https://github.com/kata-containers/kata-containers/issues/2542)
	if err := utils.ChownToParent(v.socketPath); err != nil {
		return nil, err
	}

	// no longer needed since fd is a dup
	defer listener.Close()

	listener.SetUnlinkOnClose(false)

	return listener.File()
}

// Start the virtiofsd daemon
func (v *virtiofsd) Start(ctx context.Context, onQuit onQuitFunc) (int, error) {
	span, _ := katatrace.Trace(ctx, v.Logger(), "Start", virtiofsdTracingTags)
	defer span.End()
	pid := 0

	if err := v.valid(); err != nil {
		return pid, err
	}

	cmd := exec.Command(v.path)

	socketFD, err := v.getSocketFD()
	if err != nil {
		return 0, err
	}
	defer socketFD.Close()

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
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return pid, err
	}

	if err = utils.StartCmd(cmd); err != nil {
		return pid, err
	}

	// Monitor virtiofsd's stderr and stop sandbox if virtiofsd quits
	go func() {
		scanner := bufio.NewScanner(stderr)
		for scanner.Scan() {
			v.Logger().WithField("source", "virtiofsd").Info(scanner.Text())
		}
		v.Logger().Info("virtiofsd quits")
		// Wait to release resources of virtiofsd process
		cmd.Process.Wait()
		if onQuit != nil {
			onQuit()
		}
	}()

	v.PID = cmd.Process.Pid

	return cmd.Process.Pid, nil
}

func (v *virtiofsd) Stop(ctx context.Context) error {
	if err := v.kill(ctx); err != nil {
		return nil
	}

	err := os.Remove(v.socketPath)
	if err != nil {
		v.Logger().WithError(err).WithField("path", v.socketPath).Warn("removing virtiofsd socket failed")
	}
	return nil
}

func (v *virtiofsd) args(FdSocketNumber uint) ([]string, error) {

	args := []string{
		// Send logs to syslog
		"--syslog",
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
		// enable debug output (implies -f)
		args = append(args, "-d")
	} else {
		// foreground operation
		args = append(args, "-f")
	}

	if len(v.extraArgs) != 0 {
		args = append(args, v.extraArgs...)
	}

	return args, nil
}

func (v *virtiofsd) valid() error {
	if v.path == "" {
		return errVirtiofsdDaemonPathEmpty
	}

	if v.socketPath == "" {
		return errVirtiofsdSocketPathEmpty
	}

	if v.sourcePath == "" {
		return errVirtiofsdSourcePathEmpty
	}

	if _, err := os.Stat(v.sourcePath); err != nil {
		return errVirtiofsdSourceNotAvailable
	}
	return nil
}

func (v *virtiofsd) Logger() *log.Entry {
	return hvLogger.WithField("subsystem", "virtiofsd")
}

func (v *virtiofsd) kill(ctx context.Context) (err error) {
	span, _ := katatrace.Trace(ctx, v.Logger(), "kill", virtiofsdTracingTags)
	defer span.End()

	if v.PID == 0 {
		v.Logger().WithField("invalid-virtiofsd-pid", v.PID).Warn("cannot kill virtiofsd")
		return nil
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
func (v *virtiofsdMock) Start(ctx context.Context, onQuit onQuitFunc) (int, error) {
	return 9999999, nil
}

func (v *virtiofsdMock) Stop(ctx context.Context) error {
	return nil
}
