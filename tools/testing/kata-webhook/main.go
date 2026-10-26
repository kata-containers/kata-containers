// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"flag"
	"fmt"
	"net/http"
	"os"
	"regexp"
	"strings"

	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/api/resource"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime"

	"github.com/sirupsen/logrus"
	kwhhttp "github.com/slok/kubewebhook/v2/pkg/http"
	kwhlogrus "github.com/slok/kubewebhook/v2/pkg/log/logrus"
	kwhmodel "github.com/slok/kubewebhook/v2/pkg/model"
	kwhmutating "github.com/slok/kubewebhook/v2/pkg/webhook/mutating"
)

const (
	minMemoryLimitEnvKey     = "MIN_MEMORY_LIMIT"
	defaultMemoryLimitEnvKey = "DEFAULT_MEMORY_LIMIT"
)

func getRuntimeClass(runtimeClassKey, defaultRuntimeClass string) string {
	if runtimeClass, ok := os.LookupEnv(runtimeClassKey); ok {
		return runtimeClass
	}
	return defaultRuntimeClass
}

func getMemoryLimit(envKey string) (*resource.Quantity, error) {
	value, ok := os.LookupEnv(envKey)
	if !ok {
		return nil, nil
	}

	limit, err := resource.ParseQuantity(value)
	if err != nil {
		return nil, fmt.Errorf("invalid %s %q: %w", envKey, value, err)
	}
	if limit.Sign() <= 0 {
		return nil, fmt.Errorf("invalid %s %q: must be greater than zero", envKey, value)
	}

	return &limit, nil
}

// The pod is mutated as unstructured data so that fields unknown to the
// vendored k8s.io/api are preserved instead of being dropped from the patch.
func setContainerMemoryLimits(obj *unstructured.Unstructured, field string, containers []corev1.Container, minimum *resource.Quantity) error {
	if minimum == nil {
		return nil
	}
	raw, found, err := unstructured.NestedSlice(obj.Object, "spec", field)
	if err != nil || !found {
		return err
	}
	if len(raw) != len(containers) {
		return fmt.Errorf("spec.%s: expected %d entries, got %d", field, len(containers), len(raw))
	}

	changed := false
	for i := range containers {
		limit, ok := containers[i].Resources.Limits[corev1.ResourceMemory]
		if !ok || limit.Cmp(*minimum) >= 0 {
			continue
		}
		container, ok := raw[i].(map[string]interface{})
		if !ok {
			return fmt.Errorf("spec.%s[%d] is not an object", field, i)
		}
		if err := unstructured.SetNestedField(container, minimum.String(), "resources", "limits", "memory"); err != nil {
			return err
		}
		fmt.Println("memory limit of", field, containers[i].Name, "too low, updating to", minimum.String())
		changed = true
	}

	if !changed {
		return nil
	}
	return unstructured.SetNestedSlice(obj.Object, raw, "spec", field)
}

// With static sandbox sizing Kata sizes the VM from the pod limits only, so a
// pod with a CPU limit but no memory limit gets a guest with nothing but the
// configured memory overhead. Give such pods a pod-level memory limit, which
// sizes the sandbox while leaving the container resources untouched. The limit
// also covers the memory the containers request, as pod-level requests default
// to it and must not be lower than the sum of the container requests.
func setPodMemoryLimit(obj *unstructured.Unstructured, pod *corev1.Pod, fallback *resource.Quantity) error {
	if fallback == nil {
		return nil
	}

	hasCPULimit := false
	requested := resource.Quantity{}
	if pod.Spec.Resources != nil {
		if _, ok := pod.Spec.Resources.Limits[corev1.ResourceMemory]; ok {
			return nil
		}
		_, hasCPULimit = pod.Spec.Resources.Limits[corev1.ResourceCPU]
	}
	for _, containers := range [][]corev1.Container{pod.Spec.Containers, pod.Spec.InitContainers} {
		for _, c := range containers {
			if _, ok := c.Resources.Limits[corev1.ResourceMemory]; ok {
				return nil
			}
			if _, ok := c.Resources.Limits[corev1.ResourceCPU]; ok {
				hasCPULimit = true
			}
		}
	}
	if !hasCPULimit {
		return nil
	}
	for _, c := range pod.Spec.Containers {
		if r, ok := c.Resources.Requests[corev1.ResourceMemory]; ok {
			requested.Add(r)
		}
	}

	limit := fallback.DeepCopy()
	if requested.Cmp(limit) > 0 {
		limit = requested
	}
	fmt.Println("setting pod memory limit to", limit.String())
	return unstructured.SetNestedField(obj.Object, limit.String(), "spec", "resources", "limits", "memory")
}

func annotatePodMutator(_ context.Context, ar *kwhmodel.AdmissionReview, obj metav1.Object) (*kwhmutating.MutatorResult, error) {
	u, ok := obj.(*unstructured.Unstructured)
	if !ok {
		// If not a pod just continue the mutation chain (if there is one) and don't do anything
		return &kwhmutating.MutatorResult{}, nil
	}

	pod := &corev1.Pod{}
	if err := runtime.DefaultUnstructuredConverter.FromUnstructured(u.Object, pod); err != nil {
		return nil, fmt.Errorf("decoding pod: %w", err)
	}

	// The Namespace is not always available in the pod Spec
	// specially when operators create the pods. Hence access
	// the Namespace in the actual request (vs the object)
	// https://godoc.org/k8s.io/api/admission/v1beta1#AdmissionRequest
	if whPolicy.nsBlacklist[ar.Namespace] {
		fmt.Println("blacklisted namespace: ", ar.Namespace)
		return &kwhmutating.MutatorResult{}, nil
	}
	// Check if we are only mutating pods in specific namespaces using the regular expression.
	if whPolicy.nsOnlyRegexp != nil && !whPolicy.nsOnlyRegexp.MatchString(ar.Namespace) {
		fmt.Println("namespace doesn't match re-only-namespaces: ", ar.Namespace)
		return &kwhmutating.MutatorResult{}, nil
	}

	// We cannot support --net=host in Kata
	// https://github.com/kata-containers/documentation/blob/master/Limitations.md#docker---nethost
	if pod.Spec.HostNetwork {
		fmt.Println("host network: ", pod.GetNamespace(), pod.GetName())
		return &kwhmutating.MutatorResult{}, nil
	}

	if pod.GetNamespace() == "sonobuoy" {
		fmt.Println("sonobuoy pods will not be changed to kata", pod.GetNamespace(), pod.GetName())
		return &kwhmutating.MutatorResult{}, nil
	}

	if pod.Spec.RuntimeClassName != nil {
		fmt.Println("explicit runtime: ", pod.GetNamespace(), pod.GetName(), pod.Spec.RuntimeClassName)
		return &kwhmutating.MutatorResult{}, nil
	}

	minMemoryLimit, err := getMemoryLimit(minMemoryLimitEnvKey)
	if err != nil {
		return nil, err
	}
	defaultMemoryLimit, err := getMemoryLimit(defaultMemoryLimitEnvKey)
	if err != nil {
		return nil, err
	}

	if err := setContainerMemoryLimits(u, "containers", pod.Spec.Containers, minMemoryLimit); err != nil {
		return nil, err
	}
	if err := setContainerMemoryLimits(u, "initContainers", pod.Spec.InitContainers, minMemoryLimit); err != nil {
		return nil, err
	}
	if err := setPodMemoryLimit(u, pod, defaultMemoryLimit); err != nil {
		return nil, err
	}

	// Mutate the pod
	fmt.Println("setting runtime to kata: ", pod.GetNamespace(), pod.GetName())

	runtimeClassEnvKey := "RUNTIME_CLASS"
	kataRuntimeClassName := getRuntimeClass(runtimeClassEnvKey, "kata")
	if err := unstructured.SetNestedField(u.Object, kataRuntimeClassName, "spec", "runtimeClassName"); err != nil {
		return nil, err
	}

	return &kwhmutating.MutatorResult{
		MutatedObject: u,
	}, nil
}

type config struct {
	certFile     string
	keyFile      string
	nsBlacklist  string
	nsOnlyRegexp string
}

type policy struct {
	nsBlacklist  map[string]bool
	nsOnlyRegexp *regexp.Regexp
}

var whPolicy *policy

func initFlags() *config {
	cfg := &config{}

	fl := flag.NewFlagSet(os.Args[0], flag.ExitOnError)
	fl.StringVar(&cfg.certFile, "tls-cert-file", "", "TLS certificate file")
	fl.StringVar(&cfg.keyFile, "tls-key-file", "", "TLS key file")
	fl.StringVar(&cfg.nsBlacklist, "exclude-namespaces", "", "Comma separated namespace blacklist")
	fl.StringVar(&cfg.nsOnlyRegexp, "only-namespaces-regexp", "", "Regexp namespace filter applied after --exclude-namespaces")

	fl.Parse(os.Args[1:])
	return cfg
}

func main() {
	logrusLogEntry := logrus.NewEntry(logrus.New())
	logrusLogEntry.Logger.SetLevel(logrus.DebugLevel)
	logger := kwhlogrus.NewLogrus(logrusLogEntry)

	cfg := initFlags()

	whPolicy = &policy{}
	whPolicy.nsBlacklist = make(map[string]bool)
	if cfg.nsBlacklist != "" {
		for _, s := range strings.Split(cfg.nsBlacklist, ",") {
			whPolicy.nsBlacklist[s] = true
		}
	}

	if cfg.nsOnlyRegexp != "" {
		var err error
		whPolicy.nsOnlyRegexp, err = regexp.Compile(cfg.nsOnlyRegexp)
		if err != nil {
			fmt.Fprintf(os.Stderr, "error compiling regular expression: %s", err)
			os.Exit(1)
		}
	}

	// Create our mutator
	mt := kwhmutating.MutatorFunc(annotatePodMutator)

	mcfg := kwhmutating.WebhookConfig{
		ID:      "podAnnotate",
		Obj:     &unstructured.Unstructured{},
		Mutator: mt,
		Logger:  logger,
	}
	wh, err := kwhmutating.NewWebhook(mcfg)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error creating webhook: %s", err)
		os.Exit(1)
	}

	// Get the handler for our webhook.
	whHandler, err := kwhhttp.HandlerFor(kwhhttp.HandlerConfig{Webhook: wh, Logger: logger})
	if err != nil {
		fmt.Fprintf(os.Stderr, "error creating webhook handler: %s", err)
		os.Exit(1)
	}

	port := ":8080"
	logger.Infof("Listening on %s", port)
	err = http.ListenAndServeTLS(port, cfg.certFile, cfg.keyFile, whHandler)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error serving webhook: %s", err)
		os.Exit(1)
	}
}
