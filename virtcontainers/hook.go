// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"syscall"
	"time"

	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/opentracing/opentracing-go/log"
	"github.com/sirupsen/logrus"
)

// Hook represents an OCI hook, including its required parameters.
type Hook struct {
	Path    string
	Args    []string
	Env     []string
	Timeout int
}

// Hooks gathers all existing OCI hooks list.
type Hooks struct {
	PreStartHooks  []Hook
	PostStartHooks []Hook
	PostStopHooks  []Hook
}

// Logger returns a logrus logger appropriate for logging Hooks messages
func (h *Hooks) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "hooks")
}

func buildHookState(processID int, s *Sandbox) specs.State {
	annotations := s.GetAnnotations()
	return specs.State{
		Pid:    processID,
		Bundle: annotations[vcAnnotations.BundlePathKey],
		ID:     s.id,
	}
}

func (h *Hook) trace(ctx context.Context, name string) (opentracing.Span, context.Context) {
	return traceWithSubsys(ctx, "hook", name)
}

func (h *Hooks) trace(ctx context.Context, name string) (opentracing.Span, context.Context) {
	return traceWithSubsys(ctx, "hooks", name)
}

func (h *Hook) runHook(s *Sandbox) error {
	span, _ := h.trace(s.ctx, "runHook")
	defer span.Finish()

	span.LogFields(
		log.String("hook-name", h.Path),
		log.String("hook-args", strings.Join(h.Args, " ")))

	state := buildHookState(os.Getpid(), s)
	stateJSON, err := json.Marshal(state)
	if err != nil {
		return err
	}

	var stdout, stderr bytes.Buffer
	cmd := &exec.Cmd{
		Path:   h.Path,
		Args:   h.Args,
		Env:    h.Env,
		Stdin:  bytes.NewReader(stateJSON),
		Stdout: &stdout,
		Stderr: &stderr,
	}

	err = cmd.Start()
	if err != nil {
		return err
	}

	if h.Timeout == 0 {
		err = cmd.Wait()
		if err != nil {
			return fmt.Errorf("%s: stdout: %s, stderr: %s", err, stdout.String(), stderr.String())
		}
	} else {
		done := make(chan error, 1)
		go func() {
			done <- cmd.Wait()
			close(done)
		}()

		select {
		case err := <-done:
			if err != nil {
				return fmt.Errorf("%s: stdout: %s, stderr: %s", err, stdout.String(), stderr.String())
			}
		case <-time.After(time.Duration(h.Timeout) * time.Second):
			if err := syscall.Kill(cmd.Process.Pid, syscall.SIGKILL); err != nil {
				return err
			}

			return fmt.Errorf("Hook timeout")
		}
	}

	return nil
}

func (h *Hooks) preStartHooks(s *Sandbox) error {
	span, _ := h.trace(s.ctx, "preStartHooks")
	defer span.Finish()

	if len(h.PreStartHooks) == 0 {
		return nil
	}

	for _, hook := range h.PreStartHooks {
		err := hook.runHook(s)
		if err != nil {
			h.Logger().WithFields(logrus.Fields{
				"hook-type": "pre-start",
				"error":     err,
			}).Error("hook error")

			return err
		}
	}

	return nil
}

func (h *Hooks) postStartHooks(s *Sandbox) error {
	span, _ := h.trace(s.ctx, "postStartHooks")
	defer span.Finish()

	if len(h.PostStartHooks) == 0 {
		return nil
	}

	for _, hook := range h.PostStartHooks {
		err := hook.runHook(s)
		if err != nil {
			// In case of post start hook, the error is not fatal,
			// just need to be logged.
			h.Logger().WithFields(logrus.Fields{
				"hook-type": "post-start",
				"error":     err,
			}).Info("hook error")
		}
	}

	return nil
}

func (h *Hooks) postStopHooks(s *Sandbox) error {
	span, _ := h.trace(s.ctx, "postStopHooks")
	defer span.Finish()

	if len(h.PostStopHooks) == 0 {
		return nil
	}

	for _, hook := range h.PostStopHooks {
		err := hook.runHook(s)
		if err != nil {
			// In case of post stop hook, the error is not fatal,
			// just need to be logged.
			h.Logger().WithFields(logrus.Fields{
				"hook-type": "post-stop",
				"error":     err,
			}).Info("hook error")
		}
	}

	return nil
}
