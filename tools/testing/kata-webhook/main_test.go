// Copyright (c) 2025 Red Hat Inc.
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"regexp"
	"testing"

	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/api/resource"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime"

	kwhmodel "github.com/slok/kubewebhook/v2/pkg/model"
)

func TestAnnotatePodMutator(t *testing.T) {
	tests := []struct {
		name                    string
		nsBlacklist             map[string]bool
		nsOnlyRegexp            *regexp.Regexp
		minMemory               *string
		defaultMemory           *string
		container               corev1.Container
		initContainer           corev1.Container
		podLimits               corev1.ResourceList
		wantMutatedRuntimeClass bool
		wantErr                 bool
		wantLimit               string
		wantInitLimit           string
		wantPodLimit            string
	}{
		{
			name:                    "no filters",
			nsBlacklist:             nil,
			nsOnlyRegexp:            nil,
			wantMutatedRuntimeClass: true,
		},
		{
			name:                    "matching nsBlacklist",
			nsBlacklist:             map[string]bool{"testing-namespace": true},
			nsOnlyRegexp:            nil,
			wantMutatedRuntimeClass: false,
		},
		{
			name:                    "matching nsOnlyRegexp",
			nsBlacklist:             nil,
			nsOnlyRegexp:            regexp.MustCompile("^testing-.*$"),
			wantMutatedRuntimeClass: true,
		},
		{
			name:                    "nonmatching nsOnlyRegexp",
			nsBlacklist:             nil,
			nsOnlyRegexp:            regexp.MustCompile(".*nonexisting.*"),
			wantMutatedRuntimeClass: false,
		},
		{
			name:                    "limit below minimum is raised",
			minMemory:               ptr("256Mi"),
			container:               memLimitContainer("128Mi"),
			initContainer:           memLimitContainer("128Mi"),
			wantMutatedRuntimeClass: true,
			wantLimit:               "256Mi",
			wantInitLimit:           "256Mi",
		},
		{
			name:                    "limit equal to minimum is kept",
			minMemory:               ptr("256Mi"),
			container:               memLimitContainer("256Mi"),
			initContainer:           memLimitContainer("256Mi"),
			wantMutatedRuntimeClass: true,
			wantLimit:               "256Mi",
			wantInitLimit:           "256Mi",
		},
		{
			name:                    "limit above minimum is kept",
			minMemory:               ptr("256Mi"),
			container:               memLimitContainer("512Mi"),
			initContainer:           memLimitContainer("512Mi"),
			wantMutatedRuntimeClass: true,
			wantLimit:               "512Mi",
			wantInitLimit:           "512Mi",
		},
		{
			name:                    "byte notation below minimum is raised",
			minMemory:               ptr("256Mi"),
			container:               memLimitContainer("268435455"),
			wantMutatedRuntimeClass: true,
			wantLimit:               "256Mi",
		},
		{
			name:                    "byte notation equal to minimum keeps its notation",
			minMemory:               ptr("256Mi"),
			container:               memLimitContainer("268435456"),
			wantMutatedRuntimeClass: true,
			wantLimit:               "268435456",
		},
		{
			name:                    "container without a memory limit stays unlimited",
			minMemory:               ptr("256Mi"),
			container:               cpuLimitContainer("1"),
			wantMutatedRuntimeClass: true,
			wantLimit:               "",
		},
		{
			name:                    "cpu-only container limit gets a pod-level memory limit",
			defaultMemory:           ptr("2Gi"),
			container:               cpuLimitContainer("100m"),
			initContainer:           cpuLimitContainer("100m"),
			wantMutatedRuntimeClass: true,
			wantLimit:               "",
			wantInitLimit:           "",
			wantPodLimit:            "2Gi",
		},
		{
			name:          "pod-level memory limit covers the container requests",
			defaultMemory: ptr("1Gi"),
			container: corev1.Container{
				Resources: corev1.ResourceRequirements{
					Limits:   corev1.ResourceList{corev1.ResourceCPU: resource.MustParse("1")},
					Requests: corev1.ResourceList{corev1.ResourceMemory: resource.MustParse("3Gi")},
				},
			},
			wantMutatedRuntimeClass: true,
			wantPodLimit:            "3Gi",
		},
		{
			name:                    "explicit container memory limit gets no pod-level limit",
			defaultMemory:           ptr("2Gi"),
			container:               memLimitContainer("512Mi"),
			initContainer:           cpuLimitContainer("1"),
			wantMutatedRuntimeClass: true,
			wantLimit:               "512Mi",
			wantPodLimit:            "",
		},
		{
			name:                    "unlimited pod gets no pod-level limit",
			defaultMemory:           ptr("2Gi"),
			wantMutatedRuntimeClass: true,
			wantPodLimit:            "",
		},
		{
			name:          "explicit pod-level memory limit is kept",
			defaultMemory: ptr("2Gi"),
			podLimits: corev1.ResourceList{
				corev1.ResourceCPU:    resource.MustParse("500m"),
				corev1.ResourceMemory: resource.MustParse("1Gi"),
			},
			wantMutatedRuntimeClass: true,
			wantPodLimit:            "1Gi",
		},
		{
			name:          "cpu-only pod-level limit gets the default memory limit",
			defaultMemory: ptr("2Gi"),
			podLimits: corev1.ResourceList{
				corev1.ResourceCPU: resource.MustParse("500m"),
			},
			wantMutatedRuntimeClass: true,
			wantPodLimit:            "2Gi",
		},
		{
			name:      "empty min memory returns error",
			minMemory: ptr(""),
			wantErr:   true,
		},
		{
			name:      "malformed min memory returns error",
			minMemory: ptr("not-a-quantity"),
			wantErr:   true,
		},
		{
			name:      "zero min memory returns error",
			minMemory: ptr("0"),
			wantErr:   true,
		},
		{
			name:      "negative min memory returns error",
			minMemory: ptr("-1Mi"),
			wantErr:   true,
		},
		{
			name:          "malformed default memory returns error",
			defaultMemory: ptr("not-a-quantity"),
			wantErr:       true,
		},
	}

	expectedRuntimeClass := "kata"
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			whPolicy = &policy{nsBlacklist: tt.nsBlacklist, nsOnlyRegexp: tt.nsOnlyRegexp}

			if tt.minMemory != nil {
				t.Setenv(minMemoryLimitEnvKey, *tt.minMemory)
			}
			if tt.defaultMemory != nil {
				t.Setenv(defaultMemoryLimitEnvKey, *tt.defaultMemory)
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
			if tt.podLimits != nil {
				pod.Spec.Resources = &corev1.ResourceRequirements{Limits: tt.podLimits}
			}

			ar := &kwhmodel.AdmissionReview{
				Namespace: "testing-namespace",
			}

			result, err := annotatePodMutator(context.Background(), ar, toUnstructured(t, pod))
			if (err != nil) != tt.wantErr {
				t.Fatalf("expected error: %v, got: %v", tt.wantErr, err)
			}
			if err != nil {
				return
			}

			mutated := pod
			if result.MutatedObject != nil {
				mutated = fromUnstructured(t, result.MutatedObject)
			}

			mutatedRuntimeClass := mutated.Spec.RuntimeClassName != nil
			if mutatedRuntimeClass != tt.wantMutatedRuntimeClass {
				t.Errorf("expected mutation: %v, got: %v", tt.wantMutatedRuntimeClass, mutatedRuntimeClass)
			}
			if mutatedRuntimeClass && *mutated.Spec.RuntimeClassName != expectedRuntimeClass {
				t.Errorf("expected runtimeclass: %v, got %v", expectedRuntimeClass, *mutated.Spec.RuntimeClassName)
			}

			assertMemoryLimit(t, mutated.Spec.Containers[0].Resources.Limits, tt.wantLimit)
			assertMemoryLimit(t, mutated.Spec.InitContainers[0].Resources.Limits, tt.wantInitLimit)
			var podLimits corev1.ResourceList
			if mutated.Spec.Resources != nil {
				podLimits = mutated.Spec.Resources.Limits
			}
			assertMemoryLimit(t, podLimits, tt.wantPodLimit)
		})
	}
}

func TestAnnotatePodMutatorPreservesUnknownFields(t *testing.T) {
	whPolicy = &policy{}
	t.Setenv(defaultMemoryLimitEnvKey, "2Gi")

	obj := toUnstructured(t, &corev1.Pod{
		ObjectMeta: metav1.ObjectMeta{Namespace: "testing-namespace"},
		Spec:       corev1.PodSpec{Containers: []corev1.Container{cpuLimitContainer("1")}},
	})
	if err := unstructured.SetNestedField(obj.Object, "kept", "spec", "futureField"); err != nil {
		t.Fatal(err)
	}
	containers, _, _ := unstructured.NestedSlice(obj.Object, "spec", "containers")
	containers[0].(map[string]interface{})["futureContainerField"] = "kept"
	if err := unstructured.SetNestedSlice(obj.Object, containers, "spec", "containers"); err != nil {
		t.Fatal(err)
	}

	result, err := annotatePodMutator(context.Background(), &kwhmodel.AdmissionReview{Namespace: "testing-namespace"}, obj)
	if err != nil {
		t.Fatal(err)
	}
	mutated := result.MutatedObject.(*unstructured.Unstructured)

	if v, _, _ := unstructured.NestedString(mutated.Object, "spec", "futureField"); v != "kept" {
		t.Errorf("expected spec.futureField to be kept, got %q", v)
	}
	containers, _, _ = unstructured.NestedSlice(mutated.Object, "spec", "containers")
	if v := containers[0].(map[string]interface{})["futureContainerField"]; v != "kept" {
		t.Errorf("expected spec.containers[0].futureContainerField to be kept, got %v", v)
	}
	if v, _, _ := unstructured.NestedString(mutated.Object, "spec", "runtimeClassName"); v != "kata" {
		t.Errorf("expected runtimeClassName kata, got %q", v)
	}
}

func ptr[T any](v T) *T {
	return &v
}

func toUnstructured(t *testing.T, pod *corev1.Pod) *unstructured.Unstructured {
	t.Helper()

	obj, err := runtime.DefaultUnstructuredConverter.ToUnstructured(pod)
	if err != nil {
		t.Fatal(err)
	}
	return &unstructured.Unstructured{Object: obj}
}

func fromUnstructured(t *testing.T, obj metav1.Object) *corev1.Pod {
	t.Helper()

	pod := &corev1.Pod{}
	if err := runtime.DefaultUnstructuredConverter.FromUnstructured(obj.(*unstructured.Unstructured).Object, pod); err != nil {
		t.Fatal(err)
	}
	return pod
}

func memLimitContainer(memory string) corev1.Container {
	return corev1.Container{
		Resources: corev1.ResourceRequirements{Limits: corev1.ResourceList{
			corev1.ResourceMemory: resource.MustParse(memory),
		}},
	}
}

func cpuLimitContainer(cpu string) corev1.Container {
	return corev1.Container{
		Resources: corev1.ResourceRequirements{Limits: corev1.ResourceList{
			corev1.ResourceCPU: resource.MustParse(cpu),
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
