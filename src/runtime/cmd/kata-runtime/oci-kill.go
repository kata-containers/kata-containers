// Copyright (c) 2024 Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"fmt"
	"strconv"
	"strings"
	"syscall"

	"github.com/urfave/cli"
)

var killCLICommand = cli.Command{
	Name:      "kill",
	Usage:     "send a signal to a container (OCI)",
	ArgsUsage: "<container-id> [signal]",
	Flags: []cli.Flag{
		cli.BoolFlag{
			Name:  "all, a",
			Usage: "send the signal to all processes in the container",
		},
	},
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}
		args := c.Args()
		if len(args) < 1 {
			return fmt.Errorf("container ID must be provided")
		}
		containerID := args.First()
		sigStr := "SIGTERM"
		if len(args) >= 2 {
			sigStr = args.Get(1)
		}
		sig, err := parseSignal(sigStr)
		if err != nil {
			return err
		}
		return runKillCommand(ctx, containerID, sig, c.Bool("all"))
	},
}

func runKillCommand(ctx context.Context, containerID string, sig syscall.Signal, all bool) error {
	sandbox, err := vci.FetchSandbox(ctx, containerID)
	if err != nil {
		return fmt.Errorf("failed to fetch sandbox %q: %w", containerID, err)
	}
	defer sandbox.Release(ctx)

	if err := sandbox.KillContainer(ctx, containerID, sig, all); err != nil {
		return fmt.Errorf("failed to kill container %q: %w", containerID, err)
	}
	return nil
}

// parseSignal converts a signal name (e.g. "SIGTERM", "TERM", "15") to syscall.Signal.
func parseSignal(s string) (syscall.Signal, error) {
	s = strings.TrimPrefix(strings.ToUpper(s), "SIG")
	if num, err := strconv.Atoi(s); err == nil {
		return syscall.Signal(num), nil
	}
	sig, ok := signalMap[s]
	if !ok {
		return 0, fmt.Errorf("unknown signal %q", s)
	}
	return sig, nil
}

var signalMap = map[string]syscall.Signal{
	"HUP":    syscall.SIGHUP,
	"INT":    syscall.SIGINT,
	"QUIT":   syscall.SIGQUIT,
	"ILL":    syscall.SIGILL,
	"TRAP":   syscall.SIGTRAP,
	"ABRT":   syscall.SIGABRT,
	"BUS":    syscall.SIGBUS,
	"FPE":    syscall.SIGFPE,
	"KILL":   syscall.SIGKILL,
	"USR1":   syscall.SIGUSR1,
	"SEGV":   syscall.SIGSEGV,
	"USR2":   syscall.SIGUSR2,
	"PIPE":   syscall.SIGPIPE,
	"ALRM":   syscall.SIGALRM,
	"TERM":   syscall.SIGTERM,
	"CHLD":   syscall.SIGCHLD,
	"CONT":   syscall.SIGCONT,
	"STOP":   syscall.SIGSTOP,
	"TSTP":   syscall.SIGTSTP,
	"TTIN":   syscall.SIGTTIN,
	"TTOU":   syscall.SIGTTOU,
	"URG":    syscall.SIGURG,
	"XCPU":   syscall.SIGXCPU,
	"XFSZ":   syscall.SIGXFSZ,
	"VTALRM": syscall.SIGVTALRM,
	"PROF":   syscall.SIGPROF,
	"WINCH":  syscall.SIGWINCH,
	"IO":     syscall.SIGIO,
	"SYS":    syscall.SIGSYS,
}
