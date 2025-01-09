// Copyright (c) 2025 Red Hat Inc.
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"os"
	"regexp"
	"testing"

	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/api/resource"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	kwhmodel "github.com/slok/kubewebhook/v2/pkg/model"
)

func TestAnnotatePodMutator(t *testing.T) {
	const (
		initialLimit     = "128Mi"
		initialInitLimit = "64Mi"
		enforcedLimit    = "256Mi"
	)

	tests := []struct {
		name          string
		nsBlacklist   map[string]bool
		nsOnlyRegexp  *regexp.Regexp
		minMemory     string
		setMinMemory  bool
		container     *corev1.Container
		wantMutated   bool
		wantErr       bool
		wantLimit     string
		wantInitLimit string
	}{
		{
			name:          "no filters",
			wantMutated:   true,
			wantLimit:     initialLimit,
			wantInitLimit: initialInitLimit,
		},
		{
			name:          "matching nsBlacklist",
			wantMutated:   false,
			nsBlacklist:   map[string]bool{"testing-namespace": true},
			wantLimit:     initialLimit,
			wantInitLimit: initialInitLimit,
		},
		{
			name:          "matching nsOnlyRegexp",
			nsOnlyRegexp:  regexp.MustCompile("^testing-.*$"),
			wantMutated:   true,
			wantLimit:     initialLimit,
			wantInitLimit: initialInitLimit,
		},
		{
			name:          "nonmatching nsOnlyRegexp",
			wantMutated:   false,
			nsOnlyRegexp:  regexp.MustCompile(".*nonexisting.*"),
			wantLimit:     initialLimit,
			wantInitLimit: initialInitLimit,
		},
		{
			name:          "limit below minimum is raised",
			minMemory:     enforcedLimit,
			setMinMemory:  true,
			wantMutated:   true,
			wantLimit:     enforcedLimit,
			wantInitLimit: enforcedLimit,
		},
		{
			name:          "limit equal to minimum is kept",
			minMemory:     enforcedLimit,
			setMinMemory:  true,
			container:     memLimitContainer(enforcedLimit),
			wantMutated:   true,
			wantLimit:     enforcedLimit,
			wantInitLimit: enforcedLimit,
		},
		{
			name:          "limit above minimum is kept",
			minMemory:     enforcedLimit,
			setMinMemory:  true,
			container:     memLimitContainer("512Mi"),
			wantMutated:   true,
			wantLimit:     "512Mi",
			wantInitLimit: enforcedLimit,
		},
		{
			name:          "byte notation below minimum is raised",
			minMemory:     enforcedLimit,
			setMinMemory:  true,
			container:     memLimitContainer("268435455"),
			wantMutated:   true,
			wantLimit:     enforcedLimit,
			wantInitLimit: enforcedLimit,
		},
		{
			name:          "byte notation equal to minimum keeps its notation",
			minMemory:     enforcedLimit,
			setMinMemory:  true,
			container:     memLimitContainer("268435456"),
			wantMutated:   true,
			wantLimit:     "268435456",
			wantInitLimit: enforcedLimit,
		},
		{
			name:         "container without a memory limit stays unlimited",
			minMemory:    enforcedLimit,
			setMinMemory: true,
			container: &corev1.Container{
				Resources: corev1.ResourceRequirements{Limits: corev1.ResourceList{
					corev1.ResourceCPU: resource.MustParse("1"),
				}},
			},
			wantMutated:   true,
			wantInitLimit: enforcedLimit,
		},
		{
			name:          "empty min memory",
			setMinMemory:  true,
			wantErr:       true,
			wantLimit:     initialLimit,
			wantInitLimit: initialInitLimit,
		},
		{
			name:          "malformed min memory",
			minMemory:     "not-a-quantity",
			setMinMemory:  true,
			wantErr:       true,
			wantLimit:     initialLimit,
			wantInitLimit: initialInitLimit,
		},
		{
			name:          "zero min memory",
			minMemory:     "0",
			setMinMemory:  true,
			wantErr:       true,
			wantLimit:     initialLimit,
			wantInitLimit: initialInitLimit,
		},
		{
			name:          "negative min memory",
			minMemory:     "-1Mi",
			setMinMemory:  true,
			wantErr:       true,
			wantLimit:     initialLimit,
			wantInitLimit: initialInitLimit,
		},
	}

	expectedRuntimeClass := "kata"
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			whPolicy = &policy{nsBlacklist: tt.nsBlacklist, nsOnlyRegexp: tt.nsOnlyRegexp}

			t.Setenv(minMemoryLimitEnvKey, tt.minMemory)
			if !tt.setMinMemory {
				if err := os.Unsetenv(minMemoryLimitEnvKey); err != nil {
					t.Fatalf("failed to unset %s: %v", minMemoryLimitEnvKey, err)
				}
			}

			container := tt.container
			if container == nil {
				container = memLimitContainer(initialLimit)
			}

			pod := &corev1.Pod{
				ObjectMeta: metav1.ObjectMeta{
					Namespace: "testing-namespace",
				},
				Spec: corev1.PodSpec{
					Containers:     []corev1.Container{*container},
					InitContainers: []corev1.Container{*memLimitContainer(initialInitLimit)},
				},
			}

			ar := &kwhmodel.AdmissionReview{
				Namespace: "testing-namespace",
			}

			result, err := annotatePodMutator(context.Background(), ar, pod)
			if tt.wantErr {
				if err == nil {
					t.Errorf("expected an error for an invalid %s", minMemoryLimitEnvKey)
				}
			} else if err != nil {
				t.Errorf("unexpected error: %v", err)
			} else {
				mutated := result.MutatedObject != nil && result.MutatedObject.(*corev1.Pod).Spec.RuntimeClassName != nil
				if mutated != tt.wantMutated {
					t.Errorf("expected mutation: %v, got: %v", tt.wantMutated, mutated)
				}
				if mutated && *result.MutatedObject.(*corev1.Pod).Spec.RuntimeClassName != expectedRuntimeClass {
					t.Errorf("expected runtimeclass: %v, got %v", expectedRuntimeClass, result.MutatedObject.(*corev1.Pod).Spec.RuntimeClassName)
				}
			}

			assertMemoryLimit(t, pod.Spec.Containers[0].Resources.Limits, tt.wantLimit)
			assertMemoryLimit(t, pod.Spec.InitContainers[0].Resources.Limits, tt.wantInitLimit)
		})
	}
}

func memLimitContainer(memory string) *corev1.Container {
	return &corev1.Container{
		Resources: corev1.ResourceRequirements{Limits: corev1.ResourceList{
			corev1.ResourceMemory: resource.MustParse(memory),
		}},
	}
}

func assertMemoryLimit(t *testing.T, limits corev1.ResourceList, want string) {
	t.Helper()

	got, ok := limits[corev1.ResourceMemory]
	if ok {
		if want == "" {
			t.Errorf("expected no memory limit, got %s", got.String())
		} else if got.String() != want {
			t.Errorf("expected memory limit %s, got %s", want, got.String())
		}
	} else if want != "" {
		t.Errorf("expected memory limit %s, got none", want)
	}
}
