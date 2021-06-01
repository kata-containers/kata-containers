// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/oci"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/exporters/trace/jaeger"
	"go.opentelemetry.io/otel/label"
	"go.opentelemetry.io/otel/propagation"
	export "go.opentelemetry.io/otel/sdk/export/trace"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/trace"
	otelTrace "go.opentelemetry.io/otel/trace"
)

// kataSpanExporter is used to ensure that Jaeger logs each span.
// This is essential as it is used by:
//
// https: //github.com/kata-containers/tests/blob/master/tracing/tracing-test.sh
type kataSpanExporter struct{}

var _ export.SpanExporter = (*kataSpanExporter)(nil)

// ExportSpans exports SpanData to Jaeger.
func (e *kataSpanExporter) ExportSpans(ctx context.Context, spans []*export.SpanData) error {
	for _, span := range spans {
		kataUtilsLogger.Tracef("Reporting span %+v", span)
	}
	return nil
}

func (e *kataSpanExporter) Shutdown(ctx context.Context) error {
	return nil
}

// tracerCloser contains a copy of the closer returned by createTracer() which
// is used by stopTracing().
var tracerCloser func()

// CreateTracer create a tracer
func CreateTracer(name string, config *oci.RuntimeConfig) (func(), error) {
	if !tracing {
		otel.SetTracerProvider(trace.NewNoopTracerProvider())
		return func() {}, nil
	}

	// build kata exporter to log reporting span records
	kataExporter := &kataSpanExporter{}

	// build jaeger exporter
	collectorEndpoint := config.JaegerEndpoint
	if collectorEndpoint == "" {
		collectorEndpoint = "http://localhost:14268/api/traces"
	}

	jaegerExporter, err := jaeger.NewRawExporter(
		jaeger.WithCollectorEndpoint(collectorEndpoint,
			jaeger.WithUsername(config.JaegerUser),
			jaeger.WithPassword(config.JaegerPassword),
		), jaeger.WithProcess(jaeger.Process{
			ServiceName: name,
			Tags: []label.KeyValue{
				label.String("exporter", "jaeger"),
				label.String("lib", "opentelemetry"),
			},
		}))
	if err != nil {
		return nil, err
	}

	// build tracer provider, that combining both jaeger exporter and kata exporter.
	tp := sdktrace.NewTracerProvider(
		sdktrace.WithConfig(
			sdktrace.Config{
				DefaultSampler: sdktrace.AlwaysSample(),
			},
		),
		sdktrace.WithSyncer(kataExporter),
		sdktrace.WithSyncer(jaegerExporter),
	)

	tracerCloser = jaegerExporter.Flush

	otel.SetTracerProvider(tp)
	otel.SetTextMapPropagator(propagation.NewCompositeTextMapPropagator(propagation.TraceContext{}, propagation.Baggage{}))
	return tracerCloser, nil
}

// StopTracing ends all tracing, reporting the spans to the collector.
func StopTracing(ctx context.Context) {
	if !tracing {
		return
	}

	span := otelTrace.SpanFromContext(ctx)
	if span != nil {
		span.End()
	}

	// report all possible spans to the collector
	if tracerCloser != nil {
		tracerCloser()
	}
}

// Trace creates a new tracing span based on the specified name and parent
// context and an opentelemetry label.KeyValue slice for span attributes.
func Trace(parent context.Context, name string, tags ...label.KeyValue) (otelTrace.Span, context.Context) {

	tracer := otel.Tracer("kata")
	ctx, span := tracer.Start(parent, name, otelTrace.WithAttributes(tags...))

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
