// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"encoding/json"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/stretchr/testify/require"
)

func TestEnvdRejectsInvalidRequests(t *testing.T) {
	for _, raw := range []string{
		"", "null", "[]", "{", "{}",
		`{"args":[]}`, `{"port":49983}`,
		`{"port":0,"args":[]}`, `{"port":65536,"args":[]}`,
		`{"port":-1,"args":[]}`, `{"port":1.5,"args":[]}`,
		`{"port":null,"args":[]}`, `{"port":49983,"args":null}`,
		`{"port":49983,"args":"serve"}`, `{"port":49983,"args":[null]}`,
		`{"port":49983,"args":["\u0000"]}`,
	} {
		t.Run(raw, func(t *testing.T) {
			_, err := parseEnvd(raw)
			require.Error(t, err)
		})
	}
}

func envdTestContainer(id string, kind ContainerType) *Container {
	return &Container{id: id, config: &ContainerConfig{
		Annotations: map[string]string{annotations.ContainerTypeKey: string(kind)},
	}}
}

func TestEnvdSelectsFirstWorkload(t *testing.T) {
	for _, tc := range []struct {
		name     string
		kind     ContainerType
		previous []ContainerType
		want     bool
	}{
		{"standalone", SingleContainer, nil, true},
		{"unannotated", "", nil, true},
		{"pause", PodSandbox, nil, false},
		{"first workload", PodContainer, []ContainerType{PodSandbox}, true},
		{"later workload", PodContainer, []ContainerType{PodSandbox, PodContainer}, false},
		{"later standalone", SingleContainer, []ContainerType{SingleContainer}, false},
	} {
		t.Run(tc.name, func(t *testing.T) {
			s := &Sandbox{containers: make(map[string]*Container)}
			for i, kind := range tc.previous {
				id := string(rune('a' + i))
				s.containers[id] = envdTestContainer(id, kind)
			}
			current := envdTestContainer("current", tc.kind)
			// Some creation paths register the current container before the call.
			s.containers[current.id] = current
			require.Equal(t, tc.want, s.isFirstEnvdWorkload(current))
		})
	}
}

func TestEnvdInjectsGuestMount(t *testing.T) {
	for _, raw := range []string{
		`{"port":65535,"args":["serve","--addr=:{port}","{port}:{port}","two words","$(literal)",""]}`,
		`{"port":1,"args":[]}`,
	} {
		s := &Sandbox{config: &SandboxConfig{Annotations: map[string]string{annotations.Envd: raw}}}
		original := &grpc.Mount{Destination: "/proc", Type: "proc"}
		spec := &grpc.Spec{
			Annotations: map[string]string{annotations.Envd: "invalid workload request", "keep": "value"},
			Mounts:      []*grpc.Mount{original},
		}
		require.NoError(t, s.configureEnvd(envdTestContainer("first", SingleContainer), spec))
		require.Equal(t, "value", spec.Annotations["keep"])
		require.Equal(t, raw, s.config.Annotations[annotations.Envd])
		expected, err := parseEnvd(raw)
		require.NoError(t, err)
		var actual envdConfig
		require.NoError(t, json.Unmarshal([]byte(spec.Annotations[annotations.Envd]), &actual))
		require.Equal(t, expected, &actual)
		if actual.Port == 65535 {
			require.Equal(t, []string{"serve", "--addr=:65535", "65535:65535", "two words", "$(literal)", ""}, actual.Args)
		}
		require.Equal(t, []*grpc.Mount{original, {
			Source: envdGuestPath, Destination: envdContainerPath,
			Type: "bind", Options: []string{"bind", "ro"},
		}}, spec.Mounts)
	}
}

func TestEnvdDiscardsWorkloadRequests(t *testing.T) {
	for _, tc := range []struct {
		name     string
		request  map[string]string
		kind     ContainerType
		previous map[string]*Container
	}{
		{"disabled", nil, PodContainer, nil},
		{"pause", map[string]string{annotations.Envd: "{}"}, PodSandbox, nil},
		{"later", map[string]string{annotations.Envd: "{}"}, PodContainer,
			map[string]*Container{"previous": envdTestContainer("previous", PodContainer)}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			s := &Sandbox{config: &SandboxConfig{Annotations: tc.request}, containers: tc.previous}
			spec := &grpc.Spec{Annotations: map[string]string{annotations.Envd: `{"port":18080,"args":[]}`}}
			require.NoError(t, s.configureEnvd(envdTestContainer("current", tc.kind), spec))
			require.NotContains(t, spec.Annotations, annotations.Envd)
			require.Empty(t, spec.Mounts)
		})
	}
	// Invalid sandbox requests fail the first workload creation.
	s := &Sandbox{config: &SandboxConfig{Annotations: map[string]string{annotations.Envd: "{}"}}}
	require.Error(t, s.configureEnvd(envdTestContainer("first", PodContainer), &grpc.Spec{}))
}
