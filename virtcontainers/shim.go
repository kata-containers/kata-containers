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
	"fmt"
	"os"
	"os/exec"
	"syscall"
	"time"

	ns "github.com/containers/virtcontainers/pkg/nsenter"
	"github.com/mitchellh/mapstructure"
	"github.com/sirupsen/logrus"
)

// ShimType describes a shim type.
type ShimType string

const (
	// CCShimType is the ccShim.
	CCShimType ShimType = "ccShim"

	// NoopShimType is the noopShim.
	NoopShimType ShimType = "noopShim"

	// KataShimType is the Kata Containers shim type.
	KataShimType ShimType = "kataShim"
)

var waitForShimTimeout = 10.0
var consoleFileMode = os.FileMode(0660)

// ShimParams is the structure providing specific parameters needed
// for the execution of the shim binary.
type ShimParams struct {
	Container string
	Token     string
	URL       string
	Console   string
	Terminal  bool
	Detach    bool
	PID       int
	CreateNS  []ns.NSType
	EnterNS   []ns.Namespace
}

// ShimConfig is the structure providing specific configuration
// for shim implementations.
type ShimConfig struct {
	Path  string
	Debug bool
}

// Set sets a shim type based on the input string.
func (pType *ShimType) Set(value string) error {
	switch value {
	case "noopShim":
		*pType = NoopShimType
		return nil
	case "ccShim":
		*pType = CCShimType
		return nil
	case "kataShim":
		*pType = KataShimType
		return nil
	default:
		return fmt.Errorf("Unknown shim type %s", value)
	}
}

// String converts a shim type to a string.
func (pType *ShimType) String() string {
	switch *pType {
	case NoopShimType:
		return string(NoopShimType)
	case CCShimType:
		return string(CCShimType)
	case KataShimType:
		return string(KataShimType)
	default:
		return ""
	}
}

// newShim returns a shim from a shim type.
func newShim(pType ShimType) (shim, error) {
	switch pType {
	case NoopShimType:
		return &noopShim{}, nil
	case CCShimType:
		return &ccShim{}, nil
	case KataShimType:
		return &kataShim{}, nil
	default:
		return &noopShim{}, nil
	}
}

// newShimConfig returns a shim config from a generic PodConfig interface.
func newShimConfig(config PodConfig) interface{} {
	switch config.ShimType {
	case NoopShimType:
		return nil
	case CCShimType, KataShimType:
		var shimConfig ShimConfig
		err := mapstructure.Decode(config.ShimConfig, &shimConfig)
		if err != nil {
			return err
		}
		return shimConfig
	default:
		return nil
	}
}

func shimLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "shim")
}

func signalShim(pid int, sig syscall.Signal) error {
	if pid <= 0 {
		return nil
	}

	shimLogger().WithFields(
		logrus.Fields{
			"shim-pid":    pid,
			"shim-signal": sig,
		}).Info("Signalling shim")

	return syscall.Kill(pid, sig)
}

func stopShim(pid int) error {
	if err := signalShim(pid, syscall.SIGKILL); err != nil && err != syscall.ESRCH {
		return err
	}

	return nil
}

func prepareAndStartShim(pod *Pod, shim shim, cid, token, url string, cmd Cmd,
	createNSList []ns.NSType, enterNSList []ns.Namespace) (*Process, error) {
	process := &Process{
		Token:     token,
		StartTime: time.Now().UTC(),
	}

	shimParams := ShimParams{
		Container: cid,
		Token:     token,
		URL:       url,
		Console:   cmd.Console,
		Terminal:  cmd.Interactive,
		Detach:    cmd.Detach,
		CreateNS:  createNSList,
		EnterNS:   enterNSList,
	}

	pid, err := shim.start(*pod, shimParams)
	if err != nil {
		return nil, err
	}

	process.Pid = pid

	return process, nil
}

func startShim(args []string, params ShimParams) (int, error) {
	cmd := exec.Command(args[0], args[1:]...)

	if !params.Detach {
		cmd.Stdin = os.Stdin
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
	}

	cloneFlags := 0
	for _, nsType := range params.CreateNS {
		cloneFlags |= ns.CloneFlagsTable[nsType]
	}

	cmd.SysProcAttr = &syscall.SysProcAttr{
		Cloneflags: uintptr(cloneFlags),
	}

	var f *os.File
	var err error
	if params.Console != "" {
		f, err = os.OpenFile(params.Console, os.O_RDWR, consoleFileMode)
		if err != nil {
			return -1, err
		}

		cmd.Stdin = f
		cmd.Stdout = f
		cmd.Stderr = f
		// Create Session
		cmd.SysProcAttr.Setsid = true
		// Set Controlling terminal to Ctty
		cmd.SysProcAttr.Setctty = true
		cmd.SysProcAttr.Ctty = int(f.Fd())
	}
	defer func() {
		if f != nil {
			f.Close()
		}
	}()

	if err := ns.NsEnter(params.EnterNS, func() error {
		return cmd.Start()
	}); err != nil {
		return -1, err
	}

	return cmd.Process.Pid, nil
}

func isShimRunning(pid int) (bool, error) {
	process, err := os.FindProcess(pid)
	if err != nil {
		return false, err
	}

	if err := process.Signal(syscall.Signal(0)); err != nil {
		return false, nil
	}

	return true, nil
}

// waitForShim waits for the end of the shim unless it reaches the timeout
// first, returning an error in that case.
func waitForShim(pid int) error {
	if pid <= 0 {
		return nil
	}

	tInit := time.Now()
	for {
		running, err := isShimRunning(pid)
		if err != nil {
			return err
		}

		if !running {
			break
		}

		if time.Since(tInit).Seconds() >= waitForShimTimeout {
			return fmt.Errorf("Shim still running, timeout %f s has been reached", waitForShimTimeout)
		}

		// Let's avoid to run a too busy loop
		time.Sleep(time.Duration(100) * time.Millisecond)
	}

	return nil
}

// shim is the virtcontainers shim interface.
type shim interface {
	// start starts the shim relying on its configuration and on
	// parameters provided.
	start(pod Pod, params ShimParams) (int, error)
}
