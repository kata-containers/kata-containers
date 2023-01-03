// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os/exec"
	"syscall"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	syscallWrapper "github.com/kata-containers/kata-containers/src/runtime/pkg/syscall"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// hookTracingTags defines tags for the trace span
var hookTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "katautils",
	"subsystem": "hook",
}

// Logger returns a logrus logger appropriate for logging hook messages
func hookLogger() *logrus.Entry {
	return kataUtilsLogger.WithField("subsystem", "hook")
}

func runHook(ctx context.Context, spec specs.Spec, hook specs.Hook, cid, bundlePath string) error {
	span, _ := katatrace.Trace(ctx, hookLogger(), "runHook", hookTracingTags)
	defer span.End()
	katatrace.AddTags(span, "path", hook.Path, "args", hook.Args)

	pid, ok := ctx.Value(vc.HypervisorPidKey{}).(int)
	if !ok || pid == 0 {
		hookLogger().Info("no hypervisor pid")

		pid = syscallWrapper.Gettid()
	}
	hookLogger().Infof("hypervisor pid %v", pid)

	state := specs.State{
		Pid:         pid,
		Bundle:      bundlePath,
		ID:          cid,
		Annotations: spec.Annotations,
	}

	stateJSON, err := json.Marshal(state)
	if err != nil {
		return err
	}

	var stdout, stderr bytes.Buffer
	cmd := &exec.Cmd{
		Path:   hook.Path,
		Args:   hook.Args,
		Env:    hook.Env,
		Stdin:  bytes.NewReader(stateJSON),
		Stdout: &stdout,
		Stderr: &stderr,
	}

	if err := cmd.Start(); err != nil {
		return err
	}

	if hook.Timeout == nil {
		if err := cmd.Wait(); err != nil {
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
		case <-time.After(time.Duration(*hook.Timeout) * time.Second):
			if err := syscall.Kill(cmd.Process.Pid, syscall.SIGKILL); err != nil {
				return err
			}

			return fmt.Errorf("Hook timeout")
		}
	}

	return nil
}

func runHooks(ctx context.Context, spec specs.Spec, hooks []specs.Hook, cid, bundlePath, hookType string) error {
	span, ctx := katatrace.Trace(ctx, hookLogger(), "runHooks", hookTracingTags)
	katatrace.AddTags(span, "type", hookType)
	defer span.End()

	for _, hook := range hooks {
		if err := runHook(ctx, spec, hook, cid, bundlePath); err != nil {
			hookLogger().WithFields(logrus.Fields{
				"hook-type": hookType,
				"error":     err,
			}).Error("hook error")

			return err
		}
	}

	return nil
}

func CreateRuntimeHooks(ctx context.Context, spec specs.Spec, cid, bundlePath string) error {
	// If no hook available, nothing needs to be done.
	if spec.Hooks == nil {
		return nil
	}

	return runHooks(ctx, spec, spec.Hooks.CreateRuntime, cid, bundlePath, "createRuntime")
}

// PreStartHooks run the hooks before start container
func PreStartHooks(ctx context.Context, spec specs.Spec, cid, bundlePath string) error {
	// If no hook available, nothing needs to be done.
	if spec.Hooks == nil {
		return nil
	}

	return runHooks(ctx, spec, spec.Hooks.Prestart, cid, bundlePath, "pre-start")
}

// PostStartHooks run the hooks just after start container
func PostStartHooks(ctx context.Context, spec specs.Spec, cid, bundlePath string) error {
	// If no hook available, nothing needs to be done.
	if spec.Hooks == nil {
		return nil
	}

	return runHooks(ctx, spec, spec.Hooks.Poststart, cid, bundlePath, "post-start")
}

// PostStopHooks run the hooks after stop container
func PostStopHooks(ctx context.Context, spec specs.Spec, cid, bundlePath string) error {
	// If no hook available, nothing needs to be done.
	if spec.Hooks == nil {
		return nil
	}

	return runHooks(ctx, spec, spec.Hooks.Poststop, cid, bundlePath, "post-stop")
}
