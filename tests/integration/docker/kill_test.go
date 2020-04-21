// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"strings"
	"syscall"
	"time"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

const (
	canBeTrapped = true
)

var genericSignalMap = map[syscall.Signal]bool{
	syscall.SIGHUP:    canBeTrapped,
	syscall.SIGINT:    canBeTrapped,
	syscall.SIGQUIT:   canBeTrapped,
	syscall.SIGILL:    canBeTrapped,
	syscall.SIGTRAP:   canBeTrapped,
	syscall.SIGIOT:    canBeTrapped,
	syscall.SIGFPE:    canBeTrapped,
	syscall.SIGUSR1:   canBeTrapped,
	syscall.SIGSEGV:   canBeTrapped,
	syscall.SIGUSR2:   canBeTrapped,
	syscall.SIGPIPE:   canBeTrapped,
	syscall.SIGALRM:   canBeTrapped,
	syscall.SIGTERM:   canBeTrapped,
	syscall.SIGCHLD:   canBeTrapped,
	syscall.SIGCONT:   canBeTrapped,
	syscall.SIGTSTP:   canBeTrapped,
	syscall.SIGTTIN:   canBeTrapped,
	syscall.SIGTTOU:   canBeTrapped,
	syscall.SIGURG:    canBeTrapped,
	syscall.SIGXCPU:   canBeTrapped,
	syscall.SIGXFSZ:   canBeTrapped,
	syscall.SIGVTALRM: canBeTrapped,
	syscall.SIGPROF:   canBeTrapped,
	syscall.SIGWINCH:  canBeTrapped,
	syscall.SIGIO:     canBeTrapped,
}

func withoutSignal() TableEntry {
	// Value denoting a command interrupted by a signal (http://www.tldp.org/LDP/abs/html/exitcodes.html)
	const interruptedBySignal = 128

	expectedExitCode := interruptedBySignal + int(syscall.SIGKILL)
	return Entry(fmt.Sprintf("without a signal"), syscall.Signal(0), expectedExitCode, true)
}

func withSignalNotExitCode(signal syscall.Signal) TableEntry {
	return Entry(fmt.Sprintf("with '%d' (%s) signal, don't change the exit code", signal, signal), signal, 0, false)
}

func withGenericSignals(signalsMap map[syscall.Signal]bool) []TableEntry {
	var table []TableEntry
	var expectedExitCode int
	for signal, trap := range signalsMap {
		expectedExitCode = int(signal)
		if !trap {
			// 128 -> command interrupted by a signal
			// http://www.tldp.org/LDP/abs/html/exitcodes.html
			expectedExitCode += 128

		}
		table = append(table, Entry(fmt.Sprintf("with '%d'(%s) signal", int(signal), signal), signal, expectedExitCode, true))
	}
	return append(table, withoutSignal(), withSignalNotExitCode(syscall.SIGSTOP))
}

var _ = Describe("docker kill", func() {
	var (
		args []string
		id   string
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("killing container",
		func(signal syscall.Signal, expectedExitCode int, waitForExit bool) {
			args = []string{"--name", id, "-dt", Image, "sh", "-c"}

			switch signal {
			case syscall.SIGQUIT, syscall.SIGILL, syscall.SIGBUS, syscall.SIGFPE, syscall.SIGSEGV, syscall.SIGPIPE:
				Skip("This is not forwarded by kata-shim " +
					"https://github.com/kata-containers/shim/issues/4")
			case syscall.SIGWINCH:
			}

			trapTag := "TRAP_RUNNING"
			trapCmd := fmt.Sprintf("trap \"exit %d\" %d; echo %s", signal, signal, trapTag)
			infiniteLoop := "while :; do sleep 1; done"

			if signal > 0 {
				args = append(args, fmt.Sprintf("%s; %s", trapCmd, infiniteLoop))
			} else {
				args = append(args, infiniteLoop)
			}

			_, _, runExitCode := dockerRun(args...)
			Expect(runExitCode).To(Equal(0))

			if signal > 0 {
				exitCh := make(chan bool)

				go func() {
					for {
						// Don't check for error here since the command
						// can fail if the container is not running yet.
						logs, _ := LogsDockerContainer(id)
						if strings.Contains(logs, trapTag) {
							break
						}

						time.Sleep(time.Second)
					}

					close(exitCh)
				}()

				var err error

				select {
				case <-exitCh:
					err = nil
				case <-time.After(time.Duration(Timeout) * time.Second):
					err = fmt.Errorf("Timeout reached after %ds", Timeout)
				}

				Expect(err).ToNot(HaveOccurred())

				dockerKill("-s", fmt.Sprintf("%d", signal), id)
			} else {
				dockerKill(id)
			}

			exitCode, err := ExitCodeDockerContainer(id, waitForExit)
			Expect(err).ToNot(HaveOccurred())
			Expect(exitCode).To(Equal(expectedExitCode))
		},
		withOSSignals(genericSignalMap)...,
	)
})
