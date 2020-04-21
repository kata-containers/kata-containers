package mutating

import (
	"context"
	"encoding/json"
	"fmt"
	"reflect"

	"github.com/appscode/jsonpatch"
	opentracing "github.com/opentracing/opentracing-go"
	admissionv1beta1 "k8s.io/api/admission/v1beta1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/runtime/serializer"

	"github.com/slok/kubewebhook/pkg/log"
	"github.com/slok/kubewebhook/pkg/observability/metrics"
	"github.com/slok/kubewebhook/pkg/webhook"
	"github.com/slok/kubewebhook/pkg/webhook/internal/helpers"
	"github.com/slok/kubewebhook/pkg/webhook/internal/instrumenting"
)

// WebhookConfig is the Mutating webhook configuration.
type WebhookConfig struct {
	Name string
	Obj  metav1.Object
}

func (c *WebhookConfig) validate() error {
	errs := ""

	if c.Name == "" {
		errs = errs + "name can't be empty"
	}

	if c.Obj == nil {
		errs = errs + "; obj can't be nil"
	}

	if errs != "" {
		return fmt.Errorf("invalid configuration: %s", errs)
	}

	return nil
}

type staticWebhook struct {
	objType      reflect.Type
	deserializer runtime.Decoder
	mutator      Mutator
	cfg          WebhookConfig
	logger       log.Logger
}

// NewWebhook is a mutating webhook and will return a webhook ready for a type of resource.
// It will mutate the received resources.
// This webhook will always allow the admission of the resource, only will deny in case of error.
func NewWebhook(cfg WebhookConfig, mutator Mutator, ot opentracing.Tracer, recorder metrics.Recorder, logger log.Logger) (webhook.Webhook, error) {
	if err := cfg.validate(); err != nil {
		return nil, err
	}

	if logger == nil {
		logger = log.Dummy
	}

	if recorder == nil {
		logger.Warningf("no metrics recorder active")
		recorder = metrics.Dummy
	}

	if ot == nil {
		logger.Warningf("no tracer active")
		ot = &opentracing.NoopTracer{}
	}

	// Create a custom deserializer for the received admission review request.
	runtimeScheme := runtime.NewScheme()
	codecs := serializer.NewCodecFactory(runtimeScheme)

	// Create our webhook and wrap for instrumentation (metrics and tracing).
	return &instrumenting.Webhook{
		Webhook: &staticWebhook{
			objType:      helpers.GetK8sObjType(cfg.Obj),
			deserializer: codecs.UniversalDeserializer(),
			mutator:      mutator,
			cfg:          cfg,
			logger:       logger,
		},
		ReviewKind:      metrics.MutatingReviewKind,
		WebhookName:     cfg.Name,
		MetricsRecorder: recorder,
		Tracer:          ot,
	}, nil
}

func (w *staticWebhook) Review(ctx context.Context, ar *admissionv1beta1.AdmissionReview) *admissionv1beta1.AdmissionResponse {
	auid := ar.Request.UID

	w.logger.Debugf("reviewing request %s, named: %s/%s", auid, ar.Request.Namespace, ar.Request.Name)
	obj := helpers.NewK8sObj(w.objType)
	runtimeObj, ok := obj.(runtime.Object)
	if !ok {
		err := fmt.Errorf("could not type assert metav1.Object to runtime.Object")
		return w.toAdmissionErrorResponse(ar, err)
	}

	// Get the object.
	_, _, err := w.deserializer.Decode(ar.Request.Object.Raw, nil, runtimeObj)
	if err != nil {
		err = fmt.Errorf("error deseralizing request raw object: %s", err)
		return w.toAdmissionErrorResponse(ar, err)
	}

	// Copy the object to have the original and be able to get the patch.
	objCopy := runtimeObj.DeepCopyObject()
	mutatingObj, ok := objCopy.(metav1.Object)
	if !ok {
		err := fmt.Errorf("impossible to type assert the deep copy to metav1.Object")
		return w.toAdmissionErrorResponse(ar, err)
	}

	return w.mutatingAdmissionReview(ctx, ar, obj, mutatingObj)

}

func (w *staticWebhook) mutatingAdmissionReview(ctx context.Context, ar *admissionv1beta1.AdmissionReview, obj, copyObj metav1.Object) *admissionv1beta1.AdmissionResponse {
	auid := ar.Request.UID

	// Mutate the object.
	_, err := w.mutator.Mutate(ctx, copyObj)
	if err != nil {
		return w.toAdmissionErrorResponse(ar, err)
	}

	// Get the diff patch of the original and mutated object.
	origJSON, err := json.Marshal(obj)
	if err != nil {
		return w.toAdmissionErrorResponse(ar, err)

	}
	mutatedJSON, err := json.Marshal(copyObj)
	if err != nil {
		return w.toAdmissionErrorResponse(ar, err)
	}

	patch, err := jsonpatch.CreatePatch(origJSON, mutatedJSON)
	if err != nil {
		return w.toAdmissionErrorResponse(ar, err)
	}

	marshalledPatch, err := json.Marshal(patch)
	if err != nil {
		return w.toAdmissionErrorResponse(ar, err)
	}
	w.logger.Debugf("json patch for request %s: %s", auid, string(marshalledPatch))

	// Forge response.
	return &admissionv1beta1.AdmissionResponse{
		UID:       auid,
		Allowed:   true,
		Patch:     marshalledPatch,
		PatchType: jsonPatchType,
	}
}

func (w *staticWebhook) toAdmissionErrorResponse(ar *admissionv1beta1.AdmissionReview, err error) *admissionv1beta1.AdmissionResponse {
	return helpers.ToAdmissionErrorResponse(ar.Request.UID, err, w.logger)
}

// jsonPatchType is the type for Kubernetes responses type.
var jsonPatchType = func() *admissionv1beta1.PatchType {
	pt := admissionv1beta1.PatchTypeJSONPatch
	return &pt
}()
