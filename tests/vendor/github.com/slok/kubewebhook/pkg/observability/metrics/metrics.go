package metrics

import (
	"time"

	admissionv1beta1 "k8s.io/api/admission/v1beta1"
)

// Operation is the operation type of the admission review.
type Operation = admissionv1beta1.Operation

// ReviewKind is the kind of admission review.
type ReviewKind string

const (
	// MutatingReviewKind is a mutating review kind.
	MutatingReviewKind ReviewKind = "mutating"

	// ValidatingReviewKind is a validating review kind.
	ValidatingReviewKind ReviewKind = "validating"
)

// Recorder knows how to record metrics.
type Recorder interface {
	// IncAdmissionReview will increment in one the admission review counter.
	IncAdmissionReview(webhook, namespace, resource string, operation Operation, kind ReviewKind)
	// IncAdmissionReviewError will increment in one the admission review counter errors.
	IncAdmissionReviewError(webhook, namespace, resource string, operation Operation, kind ReviewKind)
	// ObserveAdmissionReviewDuration will observe the duration of a admission review.
	ObserveAdmissionReviewDuration(webhook, namespace, resource string, operation Operation, kind ReviewKind, start time.Time)
}

// Dummy is a dummy recorder useful for tests.
var Dummy = &dummy{}

type dummy struct{}

func (d *dummy) IncAdmissionReview(webhook, namespace, resource string, operation Operation, kind ReviewKind) {
}
func (d *dummy) IncAdmissionReviewError(webhook, namespace, resource string, operation Operation, kind ReviewKind) {
}
func (d *dummy) ObserveAdmissionReviewDuration(webhook, namespace, resource string, operation Operation, kind ReviewKind, start time.Time) {
}
