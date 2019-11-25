// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

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
	kataUtilsLogger.Error(msg)
}

func (t traceLogger) Infof(msg string, args ...interface{}) {
	kataUtilsLogger.Infof(msg, args...)
}

// CreateTracer create a tracer
func CreateTracer(name string) (opentracing.Tracer, error) {
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

		// Ensure that Jaeger logs each span.
		// This is essential as it is used by:
		//
		// https: //github.com/kata-containers/tests/blob/master/tracing/tracing-test.sh
		Reporter: &config.ReporterConfig{
			LogSpans: tracing,
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

// StopTracing ends all tracing, reporting the spans to the collector.
func StopTracing(ctx context.Context) {
	if !tracing {
		return
	}

	span := opentracing.SpanFromContext(ctx)

	if span != nil {
		span.Finish()
	}

	// report all possible spans to the collector
	if tracerCloser != nil {
		tracerCloser.Close()
	}
}

// Trace creates a new tracing span based on the specified name and parent
// context.
func Trace(parent context.Context, name string) (opentracing.Span, context.Context) {
	span, ctx := opentracing.StartSpanFromContext(parent, name)

	span.SetTag("source", "runtime")
	span.SetTag("component", "cli")

	// This is slightly confusing: when tracing is disabled, trace spans
	// are still created - but the tracer used is a NOP. Therefore, only
	// display the message when tracing is really enabled.
	if tracing {
		// This log message is *essential*: it is used by:
		// https: //github.com/kata-containers/tests/blob/master/tracing/tracing-test.sh
		kataUtilsLogger.Debugf("created span %v", span)
	}

	return span, ctx
}
