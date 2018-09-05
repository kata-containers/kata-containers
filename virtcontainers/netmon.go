// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os/exec"
	"syscall"

	"github.com/sirupsen/logrus"
)

// NetmonConfig is the structure providing specific configuration
// for the network monitor.
type NetmonConfig struct {
	Path   string
	Debug  bool
	Enable bool
}

// netmonParams is the structure providing specific parameters needed
// for the execution of the network monitor binary.
type netmonParams struct {
	netmonPath string
	debug      bool
	logLevel   string
	runtime    string
	sandboxID  string
}

func netmonLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "netmon")
}

func prepareNetMonParams(params netmonParams) ([]string, error) {
	if params.netmonPath == "" {
		return []string{}, fmt.Errorf("Netmon path is empty")
	}
	if params.runtime == "" {
		return []string{}, fmt.Errorf("Netmon runtime path is empty")
	}
	if params.sandboxID == "" {
		return []string{}, fmt.Errorf("Netmon sandbox ID is empty")
	}

	args := []string{params.netmonPath,
		"-r", params.runtime,
		"-s", params.sandboxID,
	}

	if params.debug {
		args = append(args, "-d")
	}
	if params.logLevel != "" {
		args = append(args, []string{"-log", params.logLevel}...)
	}

	return args, nil
}

func startNetmon(params netmonParams) (int, error) {
	args, err := prepareNetMonParams(params)
	if err != nil {
		return -1, err
	}

	cmd := exec.Command(args[0], args[1:]...)
	if err := cmd.Start(); err != nil {
		return -1, err
	}

	return cmd.Process.Pid, nil
}

func stopNetmon(pid int) error {
	if pid <= 0 {
		return nil
	}

	sig := syscall.SIGKILL

	netmonLogger().WithFields(
		logrus.Fields{
			"netmon-pid":    pid,
			"netmon-signal": sig,
		}).Info("Stopping netmon")

	if err := syscall.Kill(pid, sig); err != nil && err != syscall.ESRCH {
		return err
	}

	return nil
}
