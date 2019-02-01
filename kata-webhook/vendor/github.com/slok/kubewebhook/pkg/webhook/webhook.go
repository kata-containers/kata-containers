package webhook

import (
	"context"

	admissionv1beta1 "k8s.io/api/admission/v1beta1"
)

// Webhook knows how to handle the admission reviews, in other words Webhook is a dynamic
// admission webhook for Kubernetes.
type Webhook interface {
	// Review will handle the admission reviewand return the AdmissionResponse with the result of the admission
	// error, mutation...
	Review(ctx context.Context, ar *admissionv1beta1.AdmissionReview) *admissionv1beta1.AdmissionResponse
}
