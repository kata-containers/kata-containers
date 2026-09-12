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
	tests := []struct {
		name          string
		nsBlacklist   map[string]bool
		nsOnlyRegexp  *regexp.Regexp
		minMemory     *string
		container     corev1.Container
		initContainer corev1.Container
		wantMutated   bool
		wantErr       bool
		wantLimit     string
		wantInitLimit string
	}{
		{
			name:        "no filters",
			wantMutated: true,
		},
		{
			name:        "matching nsBlacklist",
			nsBlacklist: map[string]bool{"testing-namespace": true},
			wantMutated: false,
		},
		{
			name:         "matching nsOnlyRegexp",
			nsOnlyRegexp: regexp.MustCompile("^testing-.*$"),
			wantMutated:  true,
		},
		{
			name:         "nonmatching nsOnlyRegexp",
			nsOnlyRegexp: regexp.MustCompile(".*nonexisting.*"),
			wantMutated:  false,
		},
		{
			name:          "limit below minimum is raised",
			minMemory:     ptr("256Mi"),
			container:     memLimitContainer("128Mi"),
			initContainer: memLimitContainer("128Mi"),
			wantMutated:   true,
			wantLimit:     "256Mi",
			wantInitLimit: "256Mi",
		},
		{
			name:          "limit equal to minimum is kept",
			minMemory:     ptr("256Mi"),
			container:     memLimitContainer("256Mi"),
			initContainer: memLimitContainer("256Mi"),
			wantMutated:   true,
			wantLimit:     "256Mi",
			wantInitLimit: "256Mi",
		},
		{
			name:          "limit above minimum is kept",
			minMemory:     ptr("256Mi"),
			container:     memLimitContainer("512Mi"),
			initContainer: memLimitContainer("512Mi"),
			wantMutated:   true,
			wantLimit:     "512Mi",
			wantInitLimit: "512Mi",
		},
		{
			name:        "byte notation below minimum is raised",
			minMemory:   ptr("256Mi"),
			container:   memLimitContainer("268435455"),
			wantMutated: true,
			wantLimit:   "256Mi",
		},
		{
			name:        "byte notation equal to minimum keeps its notation",
			minMemory:   ptr("256Mi"),
			container:   memLimitContainer("268435456"),
			wantMutated: true,
			wantLimit:   "268435456",
		},
		{
			name:      "container without a memory limit stays unlimited",
			minMemory: ptr("256Mi"),
			container: corev1.Container{
				Resources: corev1.ResourceRequirements{Limits: corev1.ResourceList{
					corev1.ResourceCPU: resource.MustParse("1"),
				}},
			},
			wantMutated: true,
		},
		{
			name:      "empty min memory",
			minMemory: ptr(""),
			wantErr:   true,
		},
		{
			name:      "malformed min memory",
			minMemory: ptr("not-a-quantity"),
			wantErr:   true,
		},
		{
			name:      "zero min memory",
			minMemory: ptr("0"),
			wantErr:   true,
		},
		{
			name:      "negative min memory",
			minMemory: ptr("-1Mi"),
			wantErr:   true,
		},
	}

	expectedRuntimeClass := "kata"
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			whPolicy = &policy{nsBlacklist: tt.nsBlacklist, nsOnlyRegexp: tt.nsOnlyRegexp}

			if tt.minMemory != nil {
				t.Setenv(minMemoryLimitEnvKey, *tt.minMemory)
			} else {
				if err := os.Unsetenv(minMemoryLimitEnvKey); err != nil {
					t.Fatalf("failed to unset %s: %v", minMemoryLimitEnvKey, err)
				}
			}

			pod := &corev1.Pod{
				ObjectMeta: metav1.ObjectMeta{
					Namespace: "testing-namespace",
				},
				Spec: corev1.PodSpec{
					Containers:     []corev1.Container{tt.container},
					InitContainers: []corev1.Container{tt.initContainer},
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

func ptr[T any](v T) *T {
	return &v
}

func memLimitContainer(memory string) corev1.Container {
	return corev1.Container{
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
