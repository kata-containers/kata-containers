// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katatrace

import (
	"context"

	"github.com/sirupsen/logrus"
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
		kataTraceLogger.Tracef("Reporting span %+v", span)
	}
	return nil
}

func (e *kataSpanExporter) Shutdown(ctx context.Context) error {
	return nil
}

// tracerCloser contains a copy of the closer returned by createTracer() which
// is used by stopTracing().
var tracerCloser func()

var kataTraceLogger = logrus.NewEntry(logrus.New())

// tracing determines whether tracing is enabled.
var tracing bool

// SetTracing turns tracing on or off. Called by the configuration.
func SetTracing(isTracing bool) {
	tracing = isTracing
}

// JaegerConfig defines necessary Jaeger config for exporting traces.
type JaegerConfig struct {
	JaegerEndpoint string
	JaegerUser     string
	JaegerPassword string
}

// CreateTracer create a tracer
func CreateTracer(name string, config *JaegerConfig) (func(), error) {
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

// Trace creates a new tracing span based on the specified name and parent context.
// It also accepts a logger to record nil context errors and a map of tracing tags.
// Tracing tag keys and values are strings.
func Trace(parent context.Context, logger *logrus.Entry, name string, tags map[string]string) (otelTrace.Span, context.Context) {
	if parent == nil {
		if logger == nil {
			logger = kataTraceLogger
		}
		logger.WithField("type", "bug").Error("trace called before context set")
		parent = context.Background()
	}

	var otelTags []label.KeyValue
	for k, v := range tags {
		otelTags = append(otelTags, label.Key(k).String(v))
	}

	tracer := otel.Tracer("kata")
	ctx, span := tracer.Start(parent, name, otelTrace.WithAttributes(otelTags...))

	// This is slightly confusing: when tracing is disabled, trace spans
	// are still created - but the tracer used is a NOP. Therefore, only
	// display the message when tracing is really enabled.
	if tracing {
		// This log message is *essential*: it is used by:
		// https: //github.com/kata-containers/tests/blob/master/tracing/tracing-test.sh
		kataTraceLogger.Debugf("created span %v", span)
	}

	return span, ctx
}

// AddTag adds an additional key-value pair to a tracing span. This can be used to
// provide dynamic tags that are determined at runtime.
func AddTag(span otelTrace.Span, key string, value interface{}) {
	span.SetAttributes(label.Any(key, value))
}
