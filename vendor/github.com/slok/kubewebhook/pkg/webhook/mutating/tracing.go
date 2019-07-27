package mutating

import (
	"context"

	opentracing "github.com/opentracing/opentracing-go"
	opentracingext "github.com/opentracing/opentracing-go/ext"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

// TraceMutator will wrap the mutator and trace the received mutator. for example this helper
// can be used to trace each of the mutators and get what parts of the mutating chain is the
// bottleneck.
func TraceMutator(tracer opentracing.Tracer, mutatorName string, m Mutator) Mutator {
	if tracer == nil {
		tracer = &opentracing.NoopTracer{}
	}
	return &tracedMutator{
		mutator:     m,
		tracer:      tracer,
		mutatorName: mutatorName,
	}
}

type tracedMutator struct {
	mutator     Mutator
	mutatorName string
	tracer      opentracing.Tracer
}

func (m *tracedMutator) Mutate(ctx context.Context, obj metav1.Object) (bool, error) {
	span, ctx := m.createMutatorSpan(ctx)
	defer span.Finish()

	span.LogKV("event", "start_mutate")

	// Mutate.
	stop, err := m.mutator.Mutate(ctx, obj)

	if err != nil {
		opentracingext.Error.Set(span, true)
		span.LogKV(
			"event", "error",
			"message", err,
		)
		return stop, err
	}

	span.LogKV(
		"event", "end_mutate",
		"stopChain", stop,
	)

	return stop, nil
}

func (m *tracedMutator) createMutatorSpan(ctx context.Context) (opentracing.Span, context.Context) {
	var spanOpts []opentracing.StartSpanOption

	// Check if we receive a previous span or we are the root span.
	if pSpan := opentracing.SpanFromContext(ctx); pSpan != nil {
		spanOpts = append(spanOpts, opentracing.ChildOf(pSpan.Context()))
	}

	// Create a new span.
	span := m.tracer.StartSpan("mutate", spanOpts...)

	// Set span data.
	span.SetTag("kubewebhook.mutator.name", m.mutatorName)

	ctx = opentracing.ContextWithSpan(ctx, span)
	return span, ctx
}
