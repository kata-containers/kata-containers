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
	"strings"

	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	"github.com/sirupsen/logrus"
	kwhhttp "github.com/slok/kubewebhook/v2/pkg/http"
	kwhlogrus "github.com/slok/kubewebhook/v2/pkg/log/logrus"
	kwhmodel "github.com/slok/kubewebhook/v2/pkg/model"
	kwhmutating "github.com/slok/kubewebhook/v2/pkg/webhook/mutating"
)

func getRuntimeClass(runtimeClassKey, defaultRuntimeClass string) string {
	if runtimeClass, ok := os.LookupEnv(runtimeClassKey); ok {
		return runtimeClass
	}
	return defaultRuntimeClass
}

func annotatePodMutator(_ context.Context, ar *kwhmodel.AdmissionReview, obj metav1.Object) (*kwhmutating.MutatorResult, error) {
	pod, ok := obj.(*corev1.Pod)
	if !ok {
		// If not a pod just continue the mutation chain (if there is one) and don't do anything
		return &kwhmutating.MutatorResult{}, nil
	}

	// The Namespace is not always available in the pod Spec
	// specially when operators create the pods. Hence access
	// the Namespace in the actual request (vs the object)
	// https://godoc.org/k8s.io/api/admission/v1beta1#AdmissionRequest
	if whPolicy.nsBlacklist[ar.Namespace] {
		fmt.Println("blacklisted namespace: ", ar.Namespace)
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

	for i := range pod.Spec.Containers {
		if pod.Spec.Containers[i].SecurityContext != nil && pod.Spec.Containers[i].SecurityContext.Privileged != nil {
			if *pod.Spec.Containers[i].SecurityContext.Privileged {
				fmt.Println("privileged container: ", pod.GetNamespace(), pod.GetName())
				return &kwhmutating.MutatorResult{}, nil
			}
		}
	}

	if pod.Spec.RuntimeClassName != nil {
		fmt.Println("explicit runtime: ", pod.GetNamespace(), pod.GetName(), pod.Spec.RuntimeClassName)
		return &kwhmutating.MutatorResult{}, nil
	}

	// Mutate the pod
	fmt.Println("setting runtime to kata: ", pod.GetNamespace(), pod.GetName())

	runtimeClassEnvKey := "RUNTIME_CLASS"
	kataRuntimeClassName := getRuntimeClass(runtimeClassEnvKey, "kata")
	pod.Spec.RuntimeClassName = &kataRuntimeClassName

	return &kwhmutating.MutatorResult{
		MutatedObject: pod,
	}, nil
}

type config struct {
	certFile    string
	keyFile     string
	nsBlacklist string
}

type policy struct {
	nsBlacklist map[string]bool
}

var whPolicy *policy

func initFlags() *config {
	cfg := &config{}

	fl := flag.NewFlagSet(os.Args[0], flag.ExitOnError)
	fl.StringVar(&cfg.certFile, "tls-cert-file", "", "TLS certificate file")
	fl.StringVar(&cfg.keyFile, "tls-key-file", "", "TLS key file")
	fl.StringVar(&cfg.nsBlacklist, "exclude-namespaces", "", "Comma separated namespace blacklist")

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

	// Create our mutator
	mt := kwhmutating.MutatorFunc(annotatePodMutator)

	mcfg := kwhmutating.WebhookConfig{
		ID:      "podAnnotate",
		Obj:     &corev1.Pod{},
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
