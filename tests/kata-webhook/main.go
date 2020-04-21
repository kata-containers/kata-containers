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

	whhttp "github.com/slok/kubewebhook/pkg/http"
	"github.com/slok/kubewebhook/pkg/log"
	mctx "github.com/slok/kubewebhook/pkg/webhook/context"
	mutatingwh "github.com/slok/kubewebhook/pkg/webhook/mutating"
)

func annotatePodMutator(ctx context.Context, obj metav1.Object) (bool, error) {
	pod, ok := obj.(*corev1.Pod)
	if !ok {
		// If not a pod just continue the mutation chain (if there is one) and don't do anything
		return false, nil
	}

	request := mctx.GetAdmissionRequest(ctx)
	if request == nil {
		return false, nil
	}

	// The Namespace is not always available in the pod Spec
	// specially when operators create the pods. Hence access
	// the Namespace in the actual request (vs the object)
	// https://godoc.org/k8s.io/api/admission/v1beta1#AdmissionRequest
	if whPolicy.nsBlacklist[request.Namespace] {
		fmt.Println("blacklisted namespace: ", request.Namespace)
		return false, nil
	}

	// We cannot support --net=host in Kata
	// https://github.com/kata-containers/documentation/blob/master/Limitations.md#docker---nethost
	if pod.Spec.HostNetwork {
		fmt.Println("host network: ", pod.GetNamespace(), pod.GetName())
		return false, nil
	}

	for i := range pod.Spec.Containers {
		if pod.Spec.Containers[i].SecurityContext != nil {
			if *pod.Spec.Containers[i].SecurityContext.Privileged {
				fmt.Println("privileged container: ", pod.GetNamespace(), pod.GetName())
				return false, nil
			}
		}
	}

	if pod.Spec.RuntimeClassName != nil {
		fmt.Println("explicit runtime: ", pod.GetNamespace(), pod.GetName(), pod.Spec.RuntimeClassName)
		return false, nil
	}

	// Mutate the pod
	fmt.Println("setting runtime to kata: ", pod.GetNamespace(), pod.GetName())

	kataRuntimeClassName := "kata"
	pod.Spec.RuntimeClassName = &kataRuntimeClassName

	return false, nil
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
	logger := &log.Std{Debug: true}

	cfg := initFlags()

	whPolicy = &policy{}
	whPolicy.nsBlacklist = make(map[string]bool)
	if cfg.nsBlacklist != "" {
		for _, s := range strings.Split(cfg.nsBlacklist, ",") {
			whPolicy.nsBlacklist[s] = true
		}
	}

	// Create our mutator
	mt := mutatingwh.MutatorFunc(annotatePodMutator)

	mcfg := mutatingwh.WebhookConfig{
		Name: "podAnnotate",
		Obj:  &corev1.Pod{},
	}
	wh, err := mutatingwh.NewWebhook(mcfg, mt, nil, nil, logger)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error creating webhook: %s", err)
		os.Exit(1)
	}

	// Get the handler for our webhook.
	whHandler, err := whhttp.HandlerFor(wh)
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
