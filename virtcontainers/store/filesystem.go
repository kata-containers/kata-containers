// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"

	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

type filesystem struct {
	ctx context.Context

	path string
}

// Logger returns a logrus logger appropriate for logging Store filesystem messages
func (f *filesystem) logger() *logrus.Entry {
	return storeLog.WithFields(logrus.Fields{
		"subsystem": "store",
		"backend":   "filesystem",
		"path":      f.path,
	})
}

func (f *filesystem) trace(name string) (opentracing.Span, context.Context) {
	if f.ctx == nil {
		f.logger().WithField("type", "bug").Error("trace called before context set")
		f.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(f.ctx, name)

	span.SetTag("subsystem", "store")
	span.SetTag("type", "filesystem")
	span.SetTag("path", f.path)

	return span, ctx
}

func (f *filesystem) new(ctx context.Context, path string, host string) error {
	f.ctx = ctx
	f.path = path

	f.logger().Infof("New filesystem store backend for %s", path)

	return nil
}

func (f *filesystem) load(item Item, data interface{}) error {
	span, _ := f.trace("load")
	defer span.Finish()

	span.SetTag("item", item)

	return nil
}

func (f *filesystem) store(item Item, data interface{}) error {
	span, _ := f.trace("store")
	defer span.Finish()

	span.SetTag("item", item)

	return nil
}
