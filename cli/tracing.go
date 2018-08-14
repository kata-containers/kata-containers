// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"io"

	opentracing "github.com/opentracing/opentracing-go"
	"github.com/uber/jaeger-client-go/config"
)

// Implements jaeger-client-go.Logger interface
type traceLogger struct {
}

// tracerCloser contains a copy of the closer returned by createTracer() which
// is used by stopTracing().
var tracerCloser io.Closer

func (t traceLogger) Error(msg string) {
	kataLog.Error(msg)
}

func (t traceLogger) Infof(msg string, args ...interface{}) {
	kataLog.Infof(msg, args...)
}

func createTracer(name string) (opentracing.Tracer, error) {
	cfg := &config.Configuration{
		ServiceName: name,

		// If tracing is disabled, use a NOP trace implementation
		Disabled: !tracing,

		// Note that span logging reporter option cannot be enabled as
		// it pollutes the output stream which causes (atleast) the
		// "state" command to fail under Docker.
		Sampler: &config.SamplerConfig{
			Type:  "const",
			Param: 1,
		},
	}

	logger := traceLogger{}

	tracer, closer, err := cfg.NewTracer(config.Logger(logger))
	if err != nil {
		return nil, err
	}

	// save for stopTracing()'s exclusive use
	tracerCloser = closer

	// Seems to be essential to ensure non-root spans are logged
	opentracing.SetGlobalTracer(tracer)

	return tracer, nil
}

// stopTracing() ends all tracing, reporting the spans to the collector.
func stopTracing(ctx context.Context) {
	if !tracing {
		return
	}

	span := opentracing.SpanFromContext(ctx)

	if span != nil {
		span.Finish()
	}

	// report all possible spans to the collector
	tracerCloser.Close()
}
