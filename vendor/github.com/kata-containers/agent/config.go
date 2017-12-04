//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"io/ioutil"
	"strings"

	"github.com/sirupsen/logrus"
)

const (
	optionPrefix      = "agent."
	logLevelFlag      = optionPrefix + "log"
	kernelCmdlineFile = "/proc/cmdline"
)

type agentConfig struct {
	logLevel logrus.Level
}

func newConfig(level logrus.Level) agentConfig {
	return agentConfig{
		logLevel: level,
	}
}

//Get the agent configuration from kernel cmdline
func (c *agentConfig) getConfig(cmdLineFile string) error {
	if cmdLineFile == "" {
		return fmt.Errorf("Kernel cmdline file cannot be empty")
	}

	kernelCmdline, err := ioutil.ReadFile(cmdLineFile)
	if err != nil {
		return err
	}

	words := strings.Fields(string(kernelCmdline))
	for _, w := range words {
		word := string(w)
		if err := c.parseCmdlineOption(word); err != nil {
			agentLog.WithFields(logrus.Fields{
				"error":  err,
				"option": word,
			}).Warn("Failed to parse kernel option")
		}
	}

	return nil
}

func (c *agentConfig) applyConfig() {
	agentLog.Logger.SetLevel(c.logLevel)
}

//Parse a string that represents a kernel cmdline option
func (c *agentConfig) parseCmdlineOption(option string) error {
	const (
		optionPosition = iota
		valuePosition
		optionSeparator = "="
	)

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
		c.logLevel = level
	default:
		if strings.HasPrefix(split[optionPosition], optionPrefix) {
			return fmt.Errorf("Unknown option %s", split[optionPosition])
		}
	}

	return nil

}
