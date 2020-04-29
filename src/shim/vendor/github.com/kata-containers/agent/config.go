//
// Copyright (c) 2017-2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"io/ioutil"
	"strconv"
	"strings"
	"time"

	"github.com/sirupsen/logrus"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const (
	optionPrefix               = "agent."
	logLevelFlag               = optionPrefix + "log"
	logsVSockPortFlag          = optionPrefix + "log_vport"
	devModeFlag                = optionPrefix + "devmode"
	traceModeFlag              = optionPrefix + "trace"
	useVsockFlag               = optionPrefix + "use_vsock"
	debugConsoleFlag           = optionPrefix + "debug_console"
	debugConsoleVPortFlag      = optionPrefix + "debug_console_vport"
	hotplugTimeoutFlag         = optionPrefix + "hotplug_timeout"
	unifiedCgroupHierarchyFlag = optionPrefix + "unified_cgroup_hierarchy"
	containerPipeSizeFlag      = optionPrefix + "container_pipe_size"
	traceModeStatic            = "static"
	traceModeDynamic           = "dynamic"
	traceTypeIsolated          = "isolated"
	traceTypeCollated          = "collated"
	defaultTraceType           = traceTypeIsolated
)

var kernelCmdlineFile = "/proc/cmdline"

func parseKernelCmdline() error {
	if kernelCmdlineFile == "" {
		return grpcStatus.Error(codes.FailedPrecondition, "Kernel cmdline file cannot be empty")
	}

	kernelCmdline, err := ioutil.ReadFile(kernelCmdlineFile)
	if err != nil {
		return err
	}

	words := strings.Fields(string(kernelCmdline))
	for _, word := range words {
		if err := parseCmdlineOption(word); err != nil {
			agentLog.WithFields(logrus.Fields{
				"error":  err,
				"option": word,
			}).Warn("Failed to parse kernel option")
		}
	}

	return nil
}

//Parse a string that represents a kernel cmdline option
func parseCmdlineOption(option string) error {
	const (
		optionPosition = iota
		valuePosition
		optionSeparator = "="
	)

	if option == devModeFlag {
		crashOnError = true
		debug = true

		return nil
	}

	if option == debugConsoleFlag {
		debugConsole = true
		return nil
	}

	if option == traceModeFlag {
		enableTracing(traceModeStatic, defaultTraceType)
		return nil
	}

	split := strings.Split(option, optionSeparator)

	if len(split) < valuePosition+1 {
		return nil
	}

	switch split[optionPosition] {
	case logLevelFlag:
		level, err := logrus.ParseLevel(split[valuePosition])
		if err != nil {
			return err
		}
		logLevel = level
		if level == logrus.DebugLevel {
			debug = true
		}
	case logsVSockPortFlag:
		port, err := strconv.ParseUint(split[valuePosition], 10, 32)
		if err != nil {
			return err
		}
		logsVSockPort = uint32(port)
	case debugConsoleVPortFlag:
		port, err := strconv.ParseUint(split[valuePosition], 10, 32)
		if err != nil {
			return err
		}
		debugConsole = true
		debugConsoleVSockPort = uint32(port)
	case hotplugTimeoutFlag:
		timeout, err := time.ParseDuration(split[valuePosition])
		if err != nil {
			return err
		}
		// Only use the provided timeout if a positive value is provided
		if timeout > 0 {
			hotplugTimeout = timeout
		}
	case containerPipeSizeFlag:
		size, err := strconv.ParseUint(split[valuePosition], 10, 32)
		if err != nil {
			return err
		}
		containerPipeSize = uint32(size)
	case traceModeFlag:
		switch split[valuePosition] {
		case traceTypeIsolated:
			enableTracing(traceModeStatic, traceTypeIsolated)
		case traceTypeCollated:
			enableTracing(traceModeStatic, traceTypeCollated)
		}
	case useVsockFlag:
		flag, err := strconv.ParseBool(split[valuePosition])
		if err != nil {
			return err
		}
		if flag {
			agentLog.Debug("Param passed to use vsock channel")
			commCh = vsockCh
		} else {
			agentLog.Debug("Param passed to NOT use vsock channel")
			commCh = serialCh
		}
	case unifiedCgroupHierarchyFlag:
		flag, err := strconv.ParseBool(split[valuePosition])
		if err != nil {
			return err
		}
		unifiedCgroupHierarchy = flag
	default:
		if strings.HasPrefix(split[optionPosition], optionPrefix) {
			return grpcStatus.Errorf(codes.NotFound, "Unknown option %s", split[optionPosition])
		}
	}

	return nil

}

func enableTracing(traceMode, traceType string) {
	tracing = true

	// Enable in case this generates more trace spans
	debug = true

	collatedTrace = traceType == traceTypeCollated

	agentLog.WithFields(logrus.Fields{
		"trace-mode": traceMode,
		"trace-type": traceType,
	}).Info("enabled tracing")
}
