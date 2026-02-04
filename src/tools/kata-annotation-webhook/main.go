// Copyright (c) 2019 Intel Corporation
// Copyright (c) 2022 Ant Group
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

	kataAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
)

func isKataRuntimeClass(runtimeClassName string) bool {
	return strings.HasPrefix(runtimeClassName, "kata")
}

func annotatePodMutator(_ context.Context, ar *kwhmodel.AdmissionReview, obj metav1.Object) (*kwhmutating.MutatorResult, error) {
	pod, ok := obj.(*corev1.Pod)
	if !ok {
		return &kwhmutating.MutatorResult{}, nil
	}

	if pod.Spec.RuntimeClassName == nil || !isKataRuntimeClass(*pod.Spec.RuntimeClassName) {
		return &kwhmutating.MutatorResult{}, nil
	}

	eds := kataAnnotations.EmptyDirs{
		EmptyDirs: make([]*kataAnnotations.EmptyDir, 0),
	}

	for i := range pod.Spec.Volumes {
		volume := pod.Spec.Volumes[i]
		vs := volume.VolumeSource

		if vs.EmptyDir == nil {
			continue
		}

		ed := &kataAnnotations.EmptyDir{
			Name:   volume.Name,
			Medium: string(vs.EmptyDir.Medium),
		}

		if vs.EmptyDir.SizeLimit != nil {
			ed.SizeLimit = vs.EmptyDir.SizeLimit.String()
		}

		eds.EmptyDirs = append(eds.EmptyDirs, ed)
	}

	if pod.Annotations == nil {
		pod.Annotations = make(map[string]string)
	}

	annotationValue, err := eds.String()
	if err != nil {
		return nil, err
	}
	pod.Annotations[kataAnnotations.KataAnnotSandboxVolumesEmptyDirPrefix] = annotationValue

	return &kwhmutating.MutatorResult{
		MutatedObject: pod,
	}, nil
}

type config struct {
	certFile string
	keyFile  string
}

func (c *config) TLSEnabled() bool {
	return c.certFile != "" && c.keyFile != ""
}

func initFlags() *config {
	cfg := &config{}

	fl := flag.NewFlagSet(os.Args[0], flag.ExitOnError)
	fl.StringVar(&cfg.certFile, "tls-cert-file", "", "TLS certificate file")
	fl.StringVar(&cfg.keyFile, "tls-key-file", "", "TLS key file")

	fl.Parse(os.Args[1:])
	return cfg
}

func main() {
	logrusLogEntry := logrus.NewEntry(logrus.New())
	logrusLogEntry.Logger.SetLevel(logrus.DebugLevel)
	logger := kwhlogrus.NewLogrus(logrusLogEntry)

	cfg := initFlags()

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

	var port string

	if cfg.TLSEnabled() {
		port = ":443"
		logger.Infof("Listening on %s", port)
		err = http.ListenAndServeTLS(port, cfg.certFile, cfg.keyFile, whHandler)
	} else {
		port = ":8080"
		logger.Infof("Listening on %s", port)
		err = http.ListenAndServe(port, whHandler)
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "error serving webhook: %s", err)
		os.Exit(1)
	}

}
