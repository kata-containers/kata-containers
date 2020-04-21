package helpers

import (
	"reflect"
	"strings"

	admissionv1beta1 "k8s.io/api/admission/v1beta1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"

	"github.com/slok/kubewebhook/pkg/log"
)

// ToAdmissionErrorResponse transforms an error into a admission response with error.
func ToAdmissionErrorResponse(uid types.UID, err error, logger log.Logger) *admissionv1beta1.AdmissionResponse {
	logger.Errorf("admission webhook error: %s", err)
	return &admissionv1beta1.AdmissionResponse{
		UID: uid,
		Result: &metav1.Status{
			Message: err.Error(),
			Status:  metav1.StatusFailure,
		},
	}
}

// NewK8sObj returns a new object of a Kubernetes type based on the type.
func NewK8sObj(t reflect.Type) metav1.Object {
	// Create a new object of the webhook resource type
	// convert to ptr and typeassert to Kubernetes Object.
	var obj interface{}
	newObj := reflect.New(t)
	obj = newObj.Interface()
	return obj.(metav1.Object)
}

// GetK8sObjType returns the type (not the pointer type) of a kubernetes object.
func GetK8sObjType(obj metav1.Object) reflect.Type {
	// Object is an interface, is safe to assume that is a pointer.
	// Get the indirect type of the object.
	return reflect.Indirect(reflect.ValueOf(obj)).Type()
}

// GroupVersionResourceToString returns a string representation. It differs from the
// original stringer of the object itself.
func GroupVersionResourceToString(gvr metav1.GroupVersionResource) string {
	return strings.Join([]string{gvr.Group, "/", gvr.Version, "/", gvr.Resource}, "")
}
