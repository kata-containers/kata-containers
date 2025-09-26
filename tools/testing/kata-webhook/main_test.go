// Copyright (c) 2025 Red Hat Inc.
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"regexp"
	"testing"

	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	kwhmodel "github.com/slok/kubewebhook/v2/pkg/model"
)

func TestAnnotatePodMutator(t *testing.T) {
	tests := []struct {
		name         string
		nsBlacklist  map[string]bool
		nsOnlyRegexp *regexp.Regexp
		wantMutated  bool
	}{
		{"no filters", nil, nil, true},
		{"matching nsBlacklist", map[string]bool{"testing-namespace": true}, nil, false},
		{"matching nsOnlyRegexp", nil, regexp.MustCompile("^testing-.*$"), true},
		{"nonmatching nsOnlyRegexp", nil, regexp.MustCompile(".*nonexisting.*"), false},
	}

	expectedRuntimeClass := "kata"
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			whPolicy = &policy{nsBlacklist: tt.nsBlacklist, nsOnlyRegexp: tt.nsOnlyRegexp}

			pod := &corev1.Pod{
				ObjectMeta: metav1.ObjectMeta{
					Namespace: "testing-namespace",
				},
			}

			ar := &kwhmodel.AdmissionReview{
				Namespace: "testing-namespace",
			}

			result, err := annotatePodMutator(context.Background(), ar, pod)
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}

			mutated := result.MutatedObject != nil && result.MutatedObject.(*corev1.Pod).Spec.RuntimeClassName != nil
			if mutated != tt.wantMutated {
				t.Errorf("expected mutation: %v, got: %v", tt.wantMutated, mutated)
			}
			if mutated && *result.MutatedObject.(*corev1.Pod).Spec.RuntimeClassName != expectedRuntimeClass {
				t.Errorf("expected runtimeclass: %v, got %v", expectedRuntimeClass, result.MutatedObject.(*corev1.Pod).Spec.RuntimeClassName)
			}
		})
	}
}
