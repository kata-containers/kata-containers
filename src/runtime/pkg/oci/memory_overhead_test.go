// Copyright (c) 2024 Kata Containers
//
// SPDX-License-Identifier: Apache-2.0
//

package oci

import (
	"testing"

	"github.com/stretchr/testify/assert"
	specs "github.com/opencontainers/runtime-spec/specs-go"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
)

func TestMemoryOverheadAnnotation(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
		HypervisorConfig: vc.HypervisorConfig{
			EnableAnnotations: []string{"memory_overhead"},
		},
	}

	// Test valid memory overhead values
	testCases := []struct {
		name     string
		value    string
		expected uint32
		wantErr  bool
	}{
		{
			name:     "Valid zero value",
			value:    "0",
			expected: 0,
			wantErr:  false,
		},
		{
			name:     "Valid small value",
			value:    "10",
			expected: 10,
			wantErr:  false,
		},
		{
			name:     "Valid medium value",
			value:    "256",
			expected: 256,
			wantErr:  false,
		},
		{
			name:     "Valid large value",
			value:    "1024",
			expected: 1024,
			wantErr:  false,
		},
		{
			name:     "Invalid negative value",
			value:    "-1",
			expected: 0,
			wantErr:  true,
		},
		{
			name:     "Invalid non-numeric value",
			value:    "abc",
			expected: 0,
			wantErr:  true,
		},
		{
			name:     "Invalid decimal value",
			value:    "1.5",
			expected: 0,
			wantErr:  true,
		},
		{
			name:     "Invalid empty value",
			value:    "",
			expected: 0,
			wantErr:  true,
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			// Reset config for each test
			config.HypervisorConfig.MemoryOverhead = 0
			ocispec.Annotations[vcAnnotations.MemOverhead] = tc.value

			err := addHypervisorMemoryOverrides(ocispec, &config, runtimeConfig)

			if tc.wantErr {
				assert.Error(err, "Expected error for value: %s", tc.value)
			} else {
				assert.NoError(err, "Expected no error for value: %s", tc.value)
				assert.Equal(tc.expected, config.HypervisorConfig.MemoryOverhead, "MemoryOverhead should be set correctly")
			}
		})
	}
}

func TestMemoryOverheadAnnotationDisabled(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: map[string]string{
			vcAnnotations.MemOverhead: "256",
		},
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
		HypervisorConfig: vc.HypervisorConfig{
			EnableAnnotations: []string{"default_memory"}, // memory_overhead not enabled
		},
	}

	// When annotation is not enabled, it should be rejected by addAnnotations
	err := addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err, "Should error when memory_overhead annotation is not enabled")
	assert.Contains(err.Error(), "annotation", "Error should mention annotation not enabled")
}

func TestMemoryOverheadAnnotationWithOtherMemoryAnnotations(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: map[string]string{
			vcAnnotations.DefaultMemory: "512",
			vcAnnotations.MemSlots:      "4",
			vcAnnotations.MemOverhead:   "128",
		},
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
		HypervisorConfig: vc.HypervisorConfig{
			EnableAnnotations: []string{"default_memory", "memory_slots", "memory_overhead"},
		},
	}

	err := addAnnotations(ocispec, &config, runtimeConfig)
	assert.NoError(err)

	// All memory-related annotations should be set
	assert.Equal(uint32(512), config.HypervisorConfig.MemorySize, "MemorySize should be set")
	assert.Equal(uint32(4), config.HypervisorConfig.MemSlots, "MemSlots should be set")
	assert.Equal(uint32(128), config.HypervisorConfig.MemoryOverhead, "MemoryOverhead should be set")
}

func TestMemoryOverheadAnnotationLargerThanDefaultMemory(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: map[string]string{
			vcAnnotations.DefaultMemory: "512",
			vcAnnotations.MemOverhead:   "1024", // Larger than default memory
		},
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
		HypervisorConfig: vc.HypervisorConfig{
			EnableAnnotations: []string{"default_memory", "memory_overhead"},
		},
	}

	// This should not error - the annotation parsing doesn't validate the relationship
	// between memory overhead and default memory, that validation happens at runtime
	err := addAnnotations(ocispec, &config, runtimeConfig)
	assert.NoError(err)
	assert.Equal(uint32(512), config.HypervisorConfig.MemorySize, "MemorySize should be set")
	assert.Equal(uint32(1024), config.HypervisorConfig.MemoryOverhead, "MemoryOverhead should be set even if larger than default memory")
}
