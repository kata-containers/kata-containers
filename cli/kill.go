// Copyright (c) 2014,2015,2016 Docker, Inc.
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

package main

import (
	"fmt"
	"strconv"
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/urfave/cli"
)

var killCLICommand = cli.Command{
	Name:  "kill",
	Usage: "Kill sends signals to the container's init process",
	ArgsUsage: `<container-id> [signal]

   <container-id> is the name for the instance of the container
   [signal] is the signal to be sent to the init process (default: SIGTERM)

EXAMPLE:
   If the container id is "ubuntu01" the following will send a "KILL" signal
   to the init process of the "ubuntu01" container:
	 
       # ` + name + ` kill ubuntu01 KILL`,
	Flags: []cli.Flag{
		cli.BoolFlag{
			Name:  "all, a",
			Usage: "send the specified signal to all processes inside the container",
		},
	},
	Action: func(context *cli.Context) error {
		args := context.Args()
		if args.Present() == false {
			return fmt.Errorf("Missing container ID")
		}

		// If signal is provided, it has to be the second argument.
		signal := args.Get(1)
		if signal == "" {
			signal = "SIGTERM"
		}

		return kill(args.First(), signal, context.Bool("all"))
	},
}

var signals = map[string]syscall.Signal{
	"SIGABRT":   syscall.SIGABRT,
	"SIGALRM":   syscall.SIGALRM,
	"SIGBUS":    syscall.SIGBUS,
	"SIGCHLD":   syscall.SIGCHLD,
	"SIGCLD":    syscall.SIGCLD,
	"SIGCONT":   syscall.SIGCONT,
	"SIGFPE":    syscall.SIGFPE,
	"SIGHUP":    syscall.SIGHUP,
	"SIGILL":    syscall.SIGILL,
	"SIGINT":    syscall.SIGINT,
	"SIGIO":     syscall.SIGIO,
	"SIGIOT":    syscall.SIGIOT,
	"SIGKILL":   syscall.SIGKILL,
	"SIGPIPE":   syscall.SIGPIPE,
	"SIGPOLL":   syscall.SIGPOLL,
	"SIGPROF":   syscall.SIGPROF,
	"SIGPWR":    syscall.SIGPWR,
	"SIGQUIT":   syscall.SIGQUIT,
	"SIGSEGV":   syscall.SIGSEGV,
	"SIGSTKFLT": syscall.SIGSTKFLT,
	"SIGSTOP":   syscall.SIGSTOP,
	"SIGSYS":    syscall.SIGSYS,
	"SIGTERM":   syscall.SIGTERM,
	"SIGTRAP":   syscall.SIGTRAP,
	"SIGTSTP":   syscall.SIGTSTP,
	"SIGTTIN":   syscall.SIGTTIN,
	"SIGTTOU":   syscall.SIGTTOU,
	"SIGUNUSED": syscall.SIGUNUSED,
	"SIGURG":    syscall.SIGURG,
	"SIGUSR1":   syscall.SIGUSR1,
	"SIGUSR2":   syscall.SIGUSR2,
	"SIGVTALRM": syscall.SIGVTALRM,
	"SIGWINCH":  syscall.SIGWINCH,
	"SIGXCPU":   syscall.SIGXCPU,
	"SIGXFSZ":   syscall.SIGXFSZ,
}

func kill(containerID, signal string, all bool) error {
	// Checks the MUST and MUST NOT from OCI runtime specification
	status, podID, err := getExistingContainerInfo(containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	signum, err := processSignal(signal)
	if err != nil {
		return err
	}

	// container MUST be created or running
	if status.State.State != vc.StateReady && status.State.State != vc.StateRunning {
		return fmt.Errorf("Container %s not ready or running, cannot send a signal", containerID)
	}

	if err := vci.KillContainer(podID, containerID, signum, all); err != nil {
		return err
	}

	if signum != syscall.SIGKILL && signum != syscall.SIGTERM {
		return nil
	}

	_, err = vci.StopContainer(podID, containerID)
	return err
}

func processSignal(signal string) (syscall.Signal, error) {
	signum, signalOk := signals[signal]
	if signalOk {
		return signum, nil
	}

	// Support for short name signals (INT)
	signum, signalOk = signals["SIG"+signal]
	if signalOk {
		return signum, nil
	}

	// Support for numeric signals
	s, err := strconv.Atoi(signal)
	if err != nil {
		return 0, fmt.Errorf("Failed to convert signal %s to int", signal)
	}

	signum = syscall.Signal(s)
	// Check whether signal is valid or not
	for _, sig := range signals {
		if sig == signum {
			// signal is a valid signal
			return signum, nil
		}
	}

	return 0, fmt.Errorf("Signal %s is not supported", signal)
}
