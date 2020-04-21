package instrumenting

import (
	"context"
	"time"

	opentracing "github.com/opentracing/opentracing-go"
	opentracingext "github.com/opentracing/opentracing-go/ext"
	admissionv1beta1 "k8s.io/api/admission/v1beta1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	"github.com/slok/kubewebhook/pkg/observability/metrics"
	"github.com/slok/kubewebhook/pkg/webhook"
	"github.com/slok/kubewebhook/pkg/webhook/internal/helpers"
)

// Webhook is a webhook wrapper that instruments the webhook with metrics and tracing.
// To the end user this webhook is transparent but internally instrumenting as a webhook
// wrapper we split responsibility.
type Webhook struct {
	Webhook         webhook.Webhook
	WebhookName     string
	ReviewKind      metrics.ReviewKind
	MetricsRecorder metrics.Recorder
	Tracer          opentracing.Tracer
}

// Review will review using the webhook wrapping it with instrumentation.
func (w *Webhook) Review(ctx context.Context, ar *admissionv1beta1.AdmissionReview) *admissionv1beta1.AdmissionResponse {
	// Initialize metrics.
	w.incAdmissionReviewMetric(ar, false)
	start := time.Now()
	defer w.observeAdmissionReviewDuration(ar, start)

	// Create the span, add to the context and defer the finish of the span.
	span := w.createReviewSpan(ctx, ar)
	ctx = opentracing.ContextWithSpan(ctx, span)
	defer span.Finish()

	// Call the review process.
	span.LogKV("event", "start_review")
	resp := w.Webhook.Review(ctx, ar)

	// Check if we had an error on the review or it ended correctly.
	if resp.Result != nil && resp.Result.Status == metav1.StatusFailure {
		w.incAdmissionReviewMetric(ar, true)
		opentracingext.Error.Set(span, true)
		span.LogKV(
			"event", "error",
			"message", resp.Result.Message,
		)
		return resp
	}

	var msg, status string
	if resp.Result != nil {
		msg = resp.Result.Message
		status = resp.Result.Status
	}
	span.LogKV(
		"event", "end_review",
		"allowed", resp.Allowed,
		"message", msg,
		"patch", string(resp.Patch),
		"status", status,
	)

	return resp
}

func (w *Webhook) incAdmissionReviewMetric(ar *admissionv1beta1.AdmissionReview, err bool) {
	if err {
		w.MetricsRecorder.IncAdmissionReviewError(
			w.WebhookName,
			ar.Request.Namespace,
			helpers.GroupVersionResourceToString(ar.Request.Resource),
			ar.Request.Operation,
			w.ReviewKind)
	} else {
		w.MetricsRecorder.IncAdmissionReview(
			w.WebhookName,
			ar.Request.Namespace,
			helpers.GroupVersionResourceToString(ar.Request.Resource),
			ar.Request.Operation,
			w.ReviewKind)
	}
}

func (w *Webhook) observeAdmissionReviewDuration(ar *admissionv1beta1.AdmissionReview, start time.Time) {
	w.MetricsRecorder.ObserveAdmissionReviewDuration(
		w.WebhookName,
		ar.Request.Namespace,
		helpers.GroupVersionResourceToString(ar.Request.Resource),
		ar.Request.Operation,
		w.ReviewKind,
		start)
}

func (w *Webhook) createReviewSpan(ctx context.Context, ar *admissionv1beta1.AdmissionReview) opentracing.Span {
	var spanOpts []opentracing.StartSpanOption

	// Check if we receive a previous span or we are the root span.
	if pSpan := opentracing.SpanFromContext(ctx); pSpan != nil {
		spanOpts = append(spanOpts, opentracing.ChildOf(pSpan.Context()))
	}

	// Create a new span.
	span := w.Tracer.StartSpan("review", spanOpts...)

	// Set span data.
	opentracingext.Component.Set(span, "kubewebhook")
	opentracingext.SpanKindRPCServer.Set(span)
	span.SetTag("kubewebhook.webhook.kind", w.ReviewKind)
	span.SetTag("kubewebhook.webhook.name", w.WebhookName)

	span.SetTag("kubernetes.review.uid", ar.Request.UID)
	span.SetTag("kubernetes.review.namespace", ar.Request.Namespace)
	span.SetTag("kubernetes.review.name", ar.Request.Name)
	span.SetTag("kubernetes.review.objectKind", helpers.GroupVersionResourceToString(ar.Request.Resource))

	return span
}
