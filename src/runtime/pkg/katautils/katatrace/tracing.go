// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katatrace

import (
	"context"
	"encoding/json"

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
func Trace(parent context.Context, logger *logrus.Entry, name string, tags ...map[string]string) (otelTrace.Span, context.Context) {
	if parent == nil {
		if logger == nil {
			logger = kataTraceLogger
		}
		logger.WithField("type", "bug").Error("trace called before context set")
		parent = context.Background()
	}

	var otelTags []label.KeyValue
	// do not append tags if tracing is disabled
	if tracing {
		for _, tagSet := range tags {
			for k, v := range tagSet {
				otelTags = append(otelTags, label.Key(k).String(v))
			}
		}
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

func addTag(span otelTrace.Span, key string, value interface{}) {
	// do not append tags if tracing is disabled
	if !tracing {
		return
	}
	if value == nil {
		span.SetAttributes(label.String(key, "nil"))
		return
	}

	switch value := value.(type) {
	case string:
		span.SetAttributes(label.String(key, value))
	case bool:
		span.SetAttributes(label.Bool(key, value))
	case int:
		span.SetAttributes(label.Int(key, value))
	case int8:
		span.SetAttributes(label.Int(key, int(value)))
	case int16:
		span.SetAttributes(label.Int(key, int(value)))
	case int32:
		span.SetAttributes(label.Int32(key, value))
	case int64:
		span.SetAttributes(label.Int64(key, value))
	case uint:
		span.SetAttributes(label.Uint(key, value))
	case uint8:
		span.SetAttributes(label.Uint(key, uint(value)))
	case uint16:
		span.SetAttributes(label.Uint(key, uint(value)))
	case uint32:
		span.SetAttributes(label.Uint32(key, value))
	case uint64:
		span.SetAttributes(label.Uint64(key, value))
	case float32:
		span.SetAttributes(label.Float32(key, value))
	case float64:
		span.SetAttributes(label.Float64(key, value))
	default:
		content, err := json.Marshal(value)
		if content == nil && err == nil {
			span.SetAttributes(label.String(key, "nil"))
		} else if content != nil && err == nil {
			span.SetAttributes(label.String(key, string(content)))
		} else {
			kataTraceLogger.WithField("type", "bug").Error("span attribute value error")
		}
	}
}

// AddTag adds additional key-value pairs to a tracing span. This can be used to provide
// dynamic tags that are determined at runtime and tags with a non-string value.
// Must have an even number of keyValues with keys being strings.
func AddTags(span otelTrace.Span, keyValues ...interface{}) {
	if !tracing {
		return
	}
	if len(keyValues) < 2 {
		kataTraceLogger.WithField("type", "bug").Error("not enough inputs for attributes")
		return
	} else if len(keyValues)%2 != 0 {
		kataTraceLogger.WithField("type", "bug").Error("number of attribute keyValues is not even")
		return
	}
	for i := 0; i < len(keyValues); i++ {
		if key, ok := keyValues[i].(string); ok {
			addTag(span, key, keyValues[i+1])
		} else {
			kataTraceLogger.WithField("type", "bug").Error("key in attributes is not a string")
		}
		i++
	}
}
