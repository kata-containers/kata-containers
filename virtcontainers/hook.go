//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"time"

	specs "github.com/opencontainers/runtime-spec/specs-go"
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

func buildHookState(processID int) specs.State {
	return specs.State{
		Pid: processID,
	}
}

func (h *Hook) runHook() error {
	state := buildHookState(os.Getpid())
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
		done := make(chan error)

		go func() { done <- cmd.Wait() }()

		select {
		case err := <-done:
			if err != nil {
				return fmt.Errorf("%s: stdout: %s, stderr: %s", err, stdout.String(), stderr.String())
			}
		case <-time.After(time.Duration(h.Timeout) * time.Second):
			return fmt.Errorf("Hook timeout")
		}
	}

	return nil
}

func (h *Hooks) preStartHooks() error {
	if len(h.PreStartHooks) == 0 {
		return nil
	}

	for _, hook := range h.PreStartHooks {
		err := hook.runHook()
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

func (h *Hooks) postStartHooks() error {
	if len(h.PostStartHooks) == 0 {
		return nil
	}

	for _, hook := range h.PostStartHooks {
		err := hook.runHook()
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

func (h *Hooks) postStopHooks() error {
	if len(h.PostStopHooks) == 0 {
		return nil
	}

	for _, hook := range h.PostStopHooks {
		err := hook.runHook()
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
