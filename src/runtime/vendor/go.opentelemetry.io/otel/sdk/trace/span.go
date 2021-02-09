// Copyright The OpenTelemetry Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package trace // import "go.opentelemetry.io/otel/sdk/trace"

import (
	"context"
	"errors"
	"fmt"
	"reflect"
	"sync"
	"time"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/codes"
	"go.opentelemetry.io/otel/label"
	"go.opentelemetry.io/otel/trace"

	export "go.opentelemetry.io/otel/sdk/export/trace"
	"go.opentelemetry.io/otel/sdk/internal"
)

const (
	errorTypeKey    = label.Key("error.type")
	errorMessageKey = label.Key("error.message")
	errorEventName  = "error"
)

var emptySpanContext = trace.SpanContext{}

// span is an implementation of the OpenTelemetry Span API representing the
// individual component of a trace.
type span struct {
	// mu protects the contents of this span.
	mu sync.Mutex

	// data contains information recorded about the span.
	//
	// It will be non-nil if we are exporting the span or recording events for it.
	// Otherwise, data is nil, and the span is simply a carrier for the
	// SpanContext, so that the trace ID is propagated.
	data        *export.SpanData
	spanContext trace.SpanContext

	// attributes are capped at configured limit. When the capacity is reached an oldest entry
	// is removed to create room for a new entry.
	attributes *attributesMap

	// messageEvents are stored in FIFO queue capped by configured limit.
	messageEvents *evictedQueue

	// links are stored in FIFO queue capped by configured limit.
	links *evictedQueue

	// endOnce ensures End is only called once.
	endOnce sync.Once

	// executionTracerTaskEnd ends the execution tracer span.
	executionTracerTaskEnd func()

	// tracer is the SDK tracer that created this span.
	tracer *tracer
}

var _ trace.Span = &span{}

func (s *span) SpanContext() trace.SpanContext {
	if s == nil {
		return trace.SpanContext{}
	}
	return s.spanContext
}

func (s *span) IsRecording() bool {
	if s == nil {
		return false
	}
	return s.data != nil
}

func (s *span) SetStatus(code codes.Code, msg string) {
	if s == nil {
		return
	}
	if !s.IsRecording() {
		return
	}
	s.mu.Lock()
	s.data.StatusCode = code
	s.data.StatusMessage = msg
	s.mu.Unlock()
}

func (s *span) SetAttributes(attributes ...label.KeyValue) {
	if !s.IsRecording() {
		return
	}
	s.copyToCappedAttributes(attributes...)
}

// End ends the span.
//
// The only SpanOption currently supported is WithTimestamp which will set the
// end time for a Span's life-cycle.
//
// If this method is called while panicking an error event is added to the
// Span before ending it and the panic is continued.
func (s *span) End(options ...trace.SpanOption) {
	if s == nil {
		return
	}

	if recovered := recover(); recovered != nil {
		// Record but don't stop the panic.
		defer panic(recovered)
		s.addEvent(
			errorEventName,
			trace.WithAttributes(
				errorTypeKey.String(typeStr(recovered)),
				errorMessageKey.String(fmt.Sprint(recovered)),
			),
		)
	}

	if s.executionTracerTaskEnd != nil {
		s.executionTracerTaskEnd()
	}
	if !s.IsRecording() {
		return
	}
	config := trace.NewSpanConfig(options...)
	s.endOnce.Do(func() {
		sps, ok := s.tracer.provider.spanProcessors.Load().(spanProcessorStates)
		mustExportOrProcess := ok && len(sps) > 0
		if mustExportOrProcess {
			sd := s.makeSpanData()
			if config.Timestamp.IsZero() {
				sd.EndTime = internal.MonotonicEndTime(sd.StartTime)
			} else {
				sd.EndTime = config.Timestamp
			}
			for _, sp := range sps {
				sp.sp.OnEnd(sd)
			}
		}
	})
}

func (s *span) RecordError(err error, opts ...trace.EventOption) {
	if s == nil || err == nil || !s.IsRecording() {
		return
	}

	s.SetStatus(codes.Error, "")
	opts = append(opts, trace.WithAttributes(
		errorTypeKey.String(typeStr(err)),
		errorMessageKey.String(err.Error()),
	))
	s.addEvent(errorEventName, opts...)
}

func typeStr(i interface{}) string {
	t := reflect.TypeOf(i)
	if t.PkgPath() == "" && t.Name() == "" {
		// Likely a builtin type.
		return t.String()
	}
	return fmt.Sprintf("%s.%s", t.PkgPath(), t.Name())
}

func (s *span) Tracer() trace.Tracer {
	return s.tracer
}

func (s *span) AddEvent(name string, o ...trace.EventOption) {
	if !s.IsRecording() {
		return
	}
	s.addEvent(name, o...)
}

func (s *span) addEvent(name string, o ...trace.EventOption) {
	c := trace.NewEventConfig(o...)

	s.mu.Lock()
	defer s.mu.Unlock()
	s.messageEvents.add(export.Event{
		Name:       name,
		Attributes: c.Attributes,
		Time:       c.Timestamp,
	})
}

var errUninitializedSpan = errors.New("failed to set name on uninitialized span")

func (s *span) SetName(name string) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.data == nil {
		otel.Handle(errUninitializedSpan)
		return
	}
	s.data.Name = name
	// SAMPLING
	noParent := !s.data.ParentSpanID.IsValid()
	var ctx trace.SpanContext
	if noParent {
		ctx = trace.SpanContext{}
	} else {
		// FIXME: Where do we get the parent context from?
		ctx = s.data.SpanContext
	}
	data := samplingData{
		noParent:     noParent,
		remoteParent: s.data.HasRemoteParent,
		parent:       ctx,
		name:         name,
		cfg:          s.tracer.provider.config.Load().(*Config),
		span:         s,
		attributes:   s.data.Attributes,
		links:        s.data.Links,
		kind:         s.data.SpanKind,
	}
	sampled := makeSamplingDecision(data)

	// Adding attributes directly rather than using s.SetAttributes()
	// as s.mu is already locked and attempting to do so would deadlock.
	for _, a := range sampled.Attributes {
		s.attributes.add(a)
	}
}

func (s *span) addLink(link trace.Link) {
	if !s.IsRecording() {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	s.links.add(link)
}

// makeSpanData produces a SpanData representing the current state of the span.
// It requires that s.data is non-nil.
func (s *span) makeSpanData() *export.SpanData {
	var sd export.SpanData
	s.mu.Lock()
	defer s.mu.Unlock()
	sd = *s.data

	s.attributes.toSpanData(&sd)

	if len(s.messageEvents.queue) > 0 {
		sd.MessageEvents = s.interfaceArrayToMessageEventArray()
		sd.DroppedMessageEventCount = s.messageEvents.droppedCount
	}
	if len(s.links.queue) > 0 {
		sd.Links = s.interfaceArrayToLinksArray()
		sd.DroppedLinkCount = s.links.droppedCount
	}
	return &sd
}

func (s *span) interfaceArrayToLinksArray() []trace.Link {
	linkArr := make([]trace.Link, 0)
	for _, value := range s.links.queue {
		linkArr = append(linkArr, value.(trace.Link))
	}
	return linkArr
}

func (s *span) interfaceArrayToMessageEventArray() []export.Event {
	messageEventArr := make([]export.Event, 0)
	for _, value := range s.messageEvents.queue {
		messageEventArr = append(messageEventArr, value.(export.Event))
	}
	return messageEventArr
}

func (s *span) copyToCappedAttributes(attributes ...label.KeyValue) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, a := range attributes {
		if a.Value.Type() != label.INVALID {
			s.attributes.add(a)
		}
	}
}

func (s *span) addChild() {
	if !s.IsRecording() {
		return
	}
	s.mu.Lock()
	s.data.ChildSpanCount++
	s.mu.Unlock()
}

func startSpanInternal(ctx context.Context, tr *tracer, name string, parent trace.SpanContext, remoteParent bool, o *trace.SpanConfig) *span {
	var noParent bool
	span := &span{}
	span.spanContext = parent

	cfg := tr.provider.config.Load().(*Config)

	if parent == emptySpanContext {
		// Generate both TraceID and SpanID
		span.spanContext.TraceID, span.spanContext.SpanID = cfg.IDGenerator.NewIDs(ctx)
		noParent = true
	} else {
		// TraceID already exists, just generate a SpanID
		span.spanContext.SpanID = cfg.IDGenerator.NewSpanID(ctx, parent.TraceID)
	}
	data := samplingData{
		noParent:     noParent,
		remoteParent: remoteParent,
		parent:       parent,
		name:         name,
		cfg:          cfg,
		span:         span,
		attributes:   o.Attributes,
		links:        o.Links,
		kind:         o.SpanKind,
	}
	sampled := makeSamplingDecision(data)

	if !span.spanContext.IsSampled() && !o.Record {
		return span
	}

	startTime := o.Timestamp
	if startTime.IsZero() {
		startTime = time.Now()
	}
	span.data = &export.SpanData{
		SpanContext:            span.spanContext,
		StartTime:              startTime,
		SpanKind:               trace.ValidateSpanKind(o.SpanKind),
		Name:                   name,
		HasRemoteParent:        remoteParent,
		Resource:               cfg.Resource,
		InstrumentationLibrary: tr.instrumentationLibrary,
	}
	span.attributes = newAttributesMap(cfg.MaxAttributesPerSpan)
	span.messageEvents = newEvictedQueue(cfg.MaxEventsPerSpan)
	span.links = newEvictedQueue(cfg.MaxLinksPerSpan)

	span.SetAttributes(sampled.Attributes...)

	if !noParent {
		span.data.ParentSpanID = parent.SpanID
	}

	return span
}

type samplingData struct {
	noParent     bool
	remoteParent bool
	parent       trace.SpanContext
	name         string
	cfg          *Config
	span         *span
	attributes   []label.KeyValue
	links        []trace.Link
	kind         trace.SpanKind
}

func makeSamplingDecision(data samplingData) SamplingResult {
	sampler := data.cfg.DefaultSampler
	spanContext := &data.span.spanContext
	sampled := sampler.ShouldSample(SamplingParameters{
		ParentContext:   data.parent,
		TraceID:         spanContext.TraceID,
		Name:            data.name,
		HasRemoteParent: data.remoteParent,
		Kind:            data.kind,
		Attributes:      data.attributes,
		Links:           data.links,
	})
	if sampled.Decision == RecordAndSample {
		spanContext.TraceFlags |= trace.FlagsSampled
	} else {
		spanContext.TraceFlags &^= trace.FlagsSampled
	}
	return sampled
}
