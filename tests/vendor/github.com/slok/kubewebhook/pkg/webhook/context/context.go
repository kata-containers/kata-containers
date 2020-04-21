package context

import (
	"context"

	admissionv1beta1 "k8s.io/api/admission/v1beta1"
)

type contextKey string

var admissionRequestKey = contextKey("admissionRequest")

// SetAdmissionRequest will set a admission request on the context and return the new context that has
// the admission request set.
func SetAdmissionRequest(ctx context.Context, ar *admissionv1beta1.AdmissionRequest) context.Context {
	return context.WithValue(ctx, admissionRequestKey, ar)
}

// GetAdmissionRequest returns the admission request stored on the context. If there is no admission
// request on the context it will return nil.
func GetAdmissionRequest(ctx context.Context) *admissionv1beta1.AdmissionRequest {
	val := ctx.Value(admissionRequestKey)
	if ar, ok := val.(*admissionv1beta1.AdmissionRequest); ok {
		return ar
	}
	return nil
}
