// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"fmt"
	"log/syslog"
	"os"
	"os/signal"
	"sync"
	"time"

	"github.com/sirupsen/logrus"
	lSyslog "github.com/sirupsen/logrus/hooks/syslog"
)

const (
	shimName    = "kata-shim"
	exitFailure = 1
)

// version is the shim version. This variable is populated at build time.
var version = "unknown"

var shimLog = logrus.New()

func logger() *logrus.Entry {
	return shimLog.WithFields(logrus.Fields{
		"name": shimName,
		"pid":  os.Getpid(),
	})
}

func initLogger(logLevel string) error {
	shimLog.Formatter = &logrus.TextFormatter{TimestampFormat: time.RFC3339Nano}

	level, err := logrus.ParseLevel(logLevel)
	if err != nil {
		return err
	}

	shimLog.SetLevel(level)

	hook, err := lSyslog.NewSyslogHook("", "", syslog.LOG_INFO|syslog.LOG_USER, shimName)
	if err == nil {
		shimLog.AddHook(hook)
	}

	logger().WithField("version", version).Info()

	return nil
}

func main() {
	var (
		logLevel      string
		agentAddr     string
		container     string
		execID        string
		terminal      bool
		proxyExitCode bool
		showVersion   bool
	)

	flag.BoolVar(&showVersion, "version", false, "display program version and exit")
	flag.StringVar(&logLevel, "log", "warn", "set shim log level: debug, info, warn, error, fatal or panic")
	flag.StringVar(&agentAddr, "agent", "", "agent gRPC socket endpoint")

	flag.StringVar(&container, "container", "", "container id for the shim")
	flag.StringVar(&execID, "exec-id", "", "process id for the shim")
	flag.BoolVar(&terminal, "terminal", false, "specify if a terminal is setup")
	flag.BoolVar(&proxyExitCode, "proxy-exit-code", true, "proxy exit code of the process")

	flag.Parse()

	if showVersion {
		fmt.Printf("%v version %v\n", shimName, version)
		os.Exit(0)
	}

	if agentAddr == "" || container == "" || execID == "" {
		logger().WithField("agentAddr", agentAddr).WithField("container", container).WithField("exec-id", execID).Error("container ID, exec ID and agent socket endpoint must be set")
		os.Exit(exitFailure)
	}

	err := initLogger(logLevel)
	if err != nil {
		logger().WithError(err).WithField("loglevel", logLevel).Error("invalid log level")
		os.Exit(exitFailure)
	}

	shim, err := newShim(agentAddr, container, execID)
	if err != nil {
		logger().WithError(err).Error("failed to create new shim")
		os.Exit(exitFailure)
	}

	// winsize
	if terminal {
		termios, err := setupTerminal(int(os.Stdin.Fd()))
		if err != nil {
			logger().WithError(err).Error("failed to set raw terminal")
			os.Exit(exitFailure)
		}
		defer restoreTerminal(int(os.Stdin.Fd()), termios)
	}
	shim.monitorTtySize(os.Stdin)

	// signals
	sigc := shim.forwardAllSignals()
	defer signal.Stop(sigc)

	// This wait call cannot be deferred and has to wait for every
	// input/output to return before the code tries to go further
	// and wait for the process. Indeed, after the process has been
	// waited for, we cannot expect to do any more calls related to
	// this process since it is going to be removed from the agent.
	wg := &sync.WaitGroup{}
	shim.proxyStdio(wg, terminal)
	wg.Wait()

	// wait until exit
	exitcode, err := shim.wait()
	if err != nil {
		logger().WithError(err).WithField("exec-id", execID).Error("failed waiting for process")
		os.Exit(exitFailure)
	} else if proxyExitCode {
		logger().WithField("exitcode", exitcode).Info("using shim to proxy exit code")
		if exitcode != 0 {
			os.Exit(int(exitcode))
		}
	}
}
