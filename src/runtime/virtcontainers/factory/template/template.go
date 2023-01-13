//
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// template implements base vm factory with vm templating.

package template

import (
	"context"

	"github.com/sirupsen/logrus"
)

var templateLog = logrus.WithField("source", "virtcontainers/factory/template")

// Logger returns a logrus logger appropriate for logging template messages
func (t *template) Logger() *logrus.Entry {
	return templateLog.WithFields(logrus.Fields{
		"subsystem": "template",
	})
}

// SetLogger sets the logger for the factory template.
func SetLogger(ctx context.Context, logger logrus.FieldLogger) {
	fields := logrus.Fields{
		"source": "virtcontainers",
	}

	templateLog = logger.WithFields(fields)
}
