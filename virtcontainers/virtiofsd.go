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
	"os"
	"os/exec"
	"strings"
	"syscall"
	"time"

	"github.com/kata-containers/runtime/virtcontainers/utils"
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

// Start the virtiofsd daemon
func (v *virtiofsd) Start(ctx context.Context) (int, error) {
	span, _ := v.trace("Start")
	defer span.Finish()
	pid := 0

	if err := v.valid(); err != nil {
		return pid, err
	}

	args, err := v.args()
	if err != nil {
		return pid, err
	}

	v.Logger().WithField("path", v.path).Info()
	v.Logger().WithField("args", strings.Join(args, " ")).Info()

	cmd := exec.Command(v.path, args...)
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return pid, fmt.Errorf("failed to get stderr from virtiofsd command, error: %s", err)
	}

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

	return cmd.Process.Pid, v.wait(cmd, stderr, v.debug)
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

func (v *virtiofsd) args() ([]string, error) {
	if v.sourcePath == "" {
		return []string{}, errors.New("vitiofsd source path is empty")
	}

	if _, err := os.Stat(v.sourcePath); os.IsNotExist(err) {
		return nil, err
	}

	args := []string{
		"-f",
		"-o", "vhost_user_socket=" + v.socketPath,
		"-o", "source=" + v.sourcePath,
		"-o", "cache=" + v.cache}

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
