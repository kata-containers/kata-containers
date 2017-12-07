// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"os"
	"os/signal"
	"sync"
	"time"

	"github.com/moby/moby/pkg/term"
	"github.com/sirupsen/logrus"
)

const (
	shimName    = "kata-shim"
	exitFailure = 1
)

var shimLog = logrus.WithFields(logrus.Fields{
	"name": shimName,
	"pid":  os.Getpid(),
})

func initLogger(logLevel string) error {
	shimLog.Logger.Formatter = &logrus.TextFormatter{TimestampFormat: time.RFC3339Nano}

	level, err := logrus.ParseLevel(logLevel)
	if err != nil {
		return err
	}

	logrus.SetLevel(level)

	return nil
}

func main() {
	var (
		logLevel      string
		agentAddr     string
		container     string
		pid           uint
		proxyExitCode bool
	)

	flag.StringVar(&logLevel, "log", "warn", "set shim log level: debug, info, warn, error, fatal or panic")
	flag.StringVar(&agentAddr, "agent", "", "agent gRPC socket endpoint")

	flag.StringVar(&container, "container", "", "container id for the shim")
	flag.UintVar(&pid, "pid", 0, "process id for the shim")
	flag.BoolVar(&proxyExitCode, "proxy-exit-code", true, "proxy exit code of the process")

	flag.Parse()

	if agentAddr == "" || container == "" || pid == 0 {
		shimLog.WithField("agentAddr", agentAddr).WithField("container", container).WithField("pid", pid).Error("container ID, process ID and agent socket endpoint must be set")
		os.Exit(exitFailure)
	}

	err := initLogger(logLevel)
	if err != nil {
		shimLog.WithError(err).WithField("loglevel", logLevel).Error("invalid log level")
		os.Exit(exitFailure)
	}

	shim, err := newShim(agentAddr, container, uint32(pid))
	if err != nil {
		shimLog.WithError(err).Error("failed to create new shim")
		os.Exit(exitFailure)
	}

	// stdio
	wg := &sync.WaitGroup{}
	shim.proxyStdio(wg)
	defer wg.Wait()

	// winsize
	s, err := term.SetRawTerminal(os.Stdin.Fd())
	if err != nil {
		shimLog.WithError(err).Error("failed to set raw terminal")
		os.Exit(exitFailure)
	}
	defer term.RestoreTerminal(os.Stdin.Fd(), s)
	shim.monitorTtySize(os.Stdin)

	// signals
	sigc := shim.forwardAllSignals()
	defer signal.Stop(sigc)

	// wait until exit
	exitcode, err := shim.wait()
	if err != nil {
		shimLog.WithError(err).WithField("pid", pid).Error("failed waiting for process")
		os.Exit(exitFailure)
	} else if proxyExitCode {
		shimLog.WithField("exitcode", exitcode).Info("using shim to proxy exit code")
		if exitcode != 0 {
			os.Exit(int(exitcode))
		}
	}
}
