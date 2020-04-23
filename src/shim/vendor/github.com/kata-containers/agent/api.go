//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/sirupsen/logrus"
)

// Serial channel
var (
	serialChannelName = "agent.channel.0"
	virtIOPath        = "/sys/class/virtio-ports"
	devRootPath       = "/dev"
)

// VSock
const (
	vSockPort = 1024
)

var vSockDevPath = "/dev/vsock"

// Signals
const (
	// If a process terminates because of signal "n"
	// The exit code is "128 + signal_number"
	// http://tldp.org/LDP/abs/html/exitcodes.html
	exitSignalOffset = 128
)

// Global
const (
	agentName       = "kata-agent"
	defaultLogLevel = logrus.InfoLevel
	selfBinPath     = "/proc/self/exe"
)
