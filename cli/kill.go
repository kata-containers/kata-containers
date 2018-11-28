// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"fmt"
	"strconv"
	"syscall"

	"github.com/kata-containers/runtime/pkg/katautils"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/sirupsen/logrus"
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
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		args := context.Args()
		if args.Present() == false {
			return fmt.Errorf("Missing container ID")
		}

		// If signal is provided, it has to be the second argument.
		signal := args.Get(1)
		if signal == "" {
			signal = "SIGTERM"
		}

		return kill(ctx, args.First(), signal, context.Bool("all"))
	},
}

var signalList = map[string]syscall.Signal{
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

func kill(ctx context.Context, containerID, signal string, all bool) error {
	span, _ := katautils.Trace(ctx, "kill")
	defer span.Finish()

	kataLog = kataLog.WithField("container", containerID)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("container", containerID)

	// Checks the MUST and MUST NOT from OCI runtime specification
	status, sandboxID, err := getExistingContainerInfo(ctx, containerID)

	if err != nil {
		return err
	}

	containerID = status.ID

	kataLog = kataLog.WithFields(logrus.Fields{
		"container": containerID,
		"sandbox":   sandboxID,
	})

	span.SetTag("container", containerID)
	span.SetTag("sandbox", sandboxID)

	setExternalLoggers(ctx, kataLog)

	signum, err := processSignal(signal)
	if err != nil {
		return err
	}

	// container MUST be created, running or paused
	if status.State.State != vc.StateReady && status.State.State != vc.StateRunning && status.State.State != vc.StatePaused {
		return fmt.Errorf("Container %s not ready, running or paused, cannot send a signal", containerID)
	}

	if err := vci.KillContainer(ctx, sandboxID, containerID, signum, all); err != nil {
		return err
	}

	if signum != syscall.SIGKILL && signum != syscall.SIGTERM {
		return nil
	}

	containerType, err := oci.GetContainerType(status.Annotations)
	if err != nil {
		return err
	}

	switch containerType {
	case vc.PodSandbox:
		_, err = vci.StopSandbox(ctx, sandboxID)
	case vc.PodContainer:
		_, err = vci.StopContainer(ctx, sandboxID, containerID)
	default:
		return fmt.Errorf("Invalid container type found")
	}

	return err
}

func processSignal(signal string) (syscall.Signal, error) {
	signum, signalOk := signalList[signal]
	if signalOk {
		return signum, nil
	}

	// Support for short name signals (INT)
	signum, signalOk = signalList["SIG"+signal]
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
	for _, sig := range signalList {
		if sig == signum {
			// signal is a valid signal
			return signum, nil
		}
	}

	return 0, fmt.Errorf("Signal %s is not supported", signal)
}
