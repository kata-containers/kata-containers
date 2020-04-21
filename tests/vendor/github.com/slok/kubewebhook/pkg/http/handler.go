package http

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net/http"

	admissionv1beta1 "k8s.io/api/admission/v1beta1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/runtime/serializer"

	"github.com/slok/kubewebhook/pkg/webhook"
	whcontext "github.com/slok/kubewebhook/pkg/webhook/context"
)

var (
	runtimeScheme = runtime.NewScheme()
	codecs        = serializer.NewCodecFactory(runtimeScheme)
	deserializer  = codecs.UniversalDeserializer()
)

// MustHandlerFor it's the same as HandleFor but will panic instead of returning
// a error.
func MustHandlerFor(webhook webhook.Webhook) http.Handler {
	h, err := HandlerFor(webhook)
	if err != nil {
		panic(err)
	}
	return h
}

// HandlerFor returns a new http.Handler ready to handle admission reviews using a
// a webhook.
func HandlerFor(webhook webhook.Webhook) (http.Handler, error) {
	if webhook == nil {
		return nil, fmt.Errorf("webhook can't be nil")
	}

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Get webhook body with the admission review.
		var body []byte
		if r.Body != nil {
			if data, err := ioutil.ReadAll(r.Body); err == nil {
				body = data
			}
		}
		if len(body) == 0 {
			http.Error(w, "no body found", http.StatusBadRequest)
			return
		}

		ar := &admissionv1beta1.AdmissionReview{}
		if _, _, err := deserializer.Decode(body, nil, ar); err != nil {
			http.Error(w, "could not decode the admission review from the request", http.StatusBadRequest)
			return
		}

		// Set the admission request on the context.
		ctx := whcontext.SetAdmissionRequest(r.Context(), ar.Request)

		// Mutation logic.
		admissionResp := webhook.Review(ctx, ar)

		// Forge the review response.
		aResponse := admissionv1beta1.AdmissionReview{
			Response: admissionResp,
		}

		resp, err := json.Marshal(aResponse)
		if err != nil {
			http.Error(w, "error marshaling to json admission review response", http.StatusInternalServerError)
			return
		}

		// Forge the HTTP response.
		// If the received admission review has failed mark the response as failed.
		if admissionResp.Result != nil && admissionResp.Result.Status == metav1.StatusFailure {
			w.WriteHeader(http.StatusInternalServerError)
		}

		w.Header().Set("Content-Type", "application/json")

		if _, err := w.Write(resp); err != nil {
			http.Error(w, fmt.Sprintf("could not write response: %v", err), http.StatusInternalServerError)
		}
	}), nil
}
