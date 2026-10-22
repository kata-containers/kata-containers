// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
)

const (
	envdGuestPath     = "/usr/libexec/kata-containers/kata-envd"
	envdContainerPath = "/dev/.kata-containers/kata-envd"
)

type envdConfig struct {
	Port uint16   `json:"port"`
	Args []string `json:"args"`
}

func parseEnvd(raw string) (*envdConfig, error) {
	// Pointers and RawMessages distinguish missing/null fields and null array
	// elements, which encoding/json otherwise accepts as zero values.
	var request struct {
		Port *uint16           `json:"port"`
		Args []json.RawMessage `json:"args"`
	}
	if err := json.Unmarshal([]byte(raw), &request); err != nil {
		return nil, fmt.Errorf("invalid envd annotation: %w", err)
	}
	if request.Port == nil || *request.Port == 0 || request.Args == nil {
		return nil, fmt.Errorf("envd requires port between 1 and 65535 and an args array")
	}
	config := &envdConfig{Port: *request.Port, Args: make([]string, len(request.Args))}
	for i, rawArg := range request.Args {
		var arg *string
		if err := json.Unmarshal(rawArg, &arg); err != nil || arg == nil {
			return nil, fmt.Errorf("envd args must contain only strings")
		}
		if strings.ContainsRune(*arg, '\x00') {
			return nil, fmt.Errorf("envd args must not contain NUL")
		}
		config.Args[i] = strings.ReplaceAll(*arg, "{port}", strconv.Itoa(int(config.Port)))
	}
	return config, nil
}

// The caller serializes container creation and registration in the sandbox.
func (s *Sandbox) isFirstEnvdWorkload(c *Container) bool {
	if ContainerType(c.config.Annotations[annotations.ContainerTypeKey]).IsCriSandbox() {
		return false
	}
	for id, previous := range s.containers {
		if id != c.id && !ContainerType(previous.config.Annotations[annotations.ContainerTypeKey]).IsCriSandbox() {
			return false
		}
	}
	return true
}

func (s *Sandbox) configureEnvd(c *Container, spec *grpc.Spec) error {
	// Always remove the workload's annotation, even when sandbox envd is off.
	delete(spec.Annotations, annotations.Envd)
	if !s.isFirstEnvdWorkload(c) {
		return nil
	}
	raw, ok := s.config.Annotations[annotations.Envd]
	if !ok {
		return nil
	}
	config, err := parseEnvd(raw)
	if err != nil {
		return err
	}
	encoded, err := json.Marshal(config)
	if err != nil {
		return err
	}
	if spec.Annotations == nil {
		spec.Annotations = make(map[string]string)
	}
	spec.Annotations[annotations.Envd] = string(encoded)
	spec.Mounts = append(spec.Mounts, &grpc.Mount{
		Source:      envdGuestPath,
		Destination: envdContainerPath,
		Type:        "bind",
		Options:     []string{"bind", "ro"},
	})
	return nil
}
