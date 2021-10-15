// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"
	"log/syslog"
	"time"

	"github.com/sirupsen/logrus"
	lSyslog "github.com/sirupsen/logrus/hooks/syslog"
)

// Default our log level to 'Warn', rather than the logrus default
// of 'Info', which is rather noisy.
var originalLoggerLevel = logrus.WarnLevel
var kataUtilsLogger = logrus.NewEntry(logrus.New())

// SYSLOGTAG is for a consistently named syslog identifier
const SYSLOGTAG = "kata"

// SetLogger sets the logger for the factory.
func SetLogger(ctx context.Context, logger *logrus.Entry, level logrus.Level) {
	fields := logrus.Fields{
		"source": "katautils",
	}

	originalLoggerLevel = level
	kataUtilsLogger = logger.WithFields(fields)
}

// sysLogHook wraps a syslog logrus hook and a formatter to be used for all
// syslog entries.
//
// This is necessary to allow the main logger (for "--log=") to use a custom
// formatter ("--log-format=") whilst allowing the system logger to use a
// different formatter.
type sysLogHook struct {
	shook     *lSyslog.SyslogHook
	formatter logrus.Formatter
}

func (h *sysLogHook) Levels() []logrus.Level {
	return h.shook.Levels()
}

// Fire is responsible for adding a log entry to the system log. It switches
// formatter before adding the system log entry, then reverts the original log
// formatter.
func (h *sysLogHook) Fire(e *logrus.Entry) (err error) {
	formatter := e.Logger.Formatter

	e.Logger.Formatter = h.formatter

	err = h.shook.Fire(e)

	e.Logger.Formatter = formatter

	return err
}

func newSystemLogHook(network, raddr string) (*sysLogHook, error) {
	hook, err := lSyslog.NewSyslogHook(network, raddr, syslog.LOG_INFO, SYSLOGTAG)
	if err != nil {
		return nil, err
	}

	return &sysLogHook{
		formatter: &logrus.TextFormatter{
			TimestampFormat: time.RFC3339Nano,
		},
		shook: hook,
	}, nil
}

// handleSystemLog sets up the system-level logger.
func handleSystemLog(network, raddr string) error {
	hook, err := newSystemLogHook(network, raddr)
	if err != nil {
		return err
	}

	kataUtilsLogger.Logger.Hooks.Add(hook)

	return nil
}
