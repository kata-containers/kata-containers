// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"context"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
)

// factoryTracingTags defines tags for the trace span
var factoryTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "factory",
	"subsystem": "factory",
}

var factoryLogger = logrus.FieldLogger(logrus.New())

// Config is a collection of VM factory configurations.
type Config struct {
	TemplatePath    string
	VMCacheEndpoint string

	VMConfig vc.VMConfig

	Cache uint

	Template bool
	VMCache  bool
}

// SetLogger sets the logger for the factory.
func SetLogger(ctx context.Context, logger logrus.FieldLogger) {
	fields := logrus.Fields{
		"source": "virtcontainers",
	}

	factoryLogger = logger.WithFields(fields)
}

func (f *factory) log() *logrus.Entry {
	return factoryLogger.WithField("subsystem", "factory")
}
