// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"testing"

	criContainerdAnnotations "github.com/containerd/cri-containerd/pkg/annotations"
	"github.com/containerd/typeurl"

	"github.com/containerd/containerd/containers"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

func TestIsSandboxContainer(t *testing.T) {
	assert := assert.New(t)

	c := &containers.Container{}
	isc := isSandboxContainer(c)
	assert.Equal(false, isc, "should not be a sandbox container")

	spec := &specs.Spec{
		Annotations: map[string]string{},
	}

	any, err := typeurl.MarshalAny(spec)
	assert.Nil(err, "MarshalAny failed for spec")

	c.Spec = any
	// default container is a pod(sandbox) container
	isc = isSandboxContainer(c)
	assert.Equal(true, isc, "should be a sandbox container")

	testCases := []struct {
		annotationKey   string
		annotationValue string
		result          bool
	}{
		{
			annotationKey:   criContainerdAnnotations.ContainerType,
			annotationValue: "",
			result:          false,
		},
		{
			annotationKey:   criContainerdAnnotations.ContainerType,
			annotationValue: criContainerdAnnotations.ContainerTypeContainer,
			result:          false,
		},
		{
			annotationKey:   criContainerdAnnotations.ContainerType,
			annotationValue: "pod",
			result:          false,
		},
		{
			annotationKey:   criContainerdAnnotations.ContainerType,
			annotationValue: criContainerdAnnotations.ContainerTypeSandbox,
			result:          true,
		},
	}

	for _, tc := range testCases {
		spec.Annotations = map[string]string{
			tc.annotationKey: tc.annotationValue,
		}
		any, err := typeurl.MarshalAny(spec)
		assert.Nil(err, "MarshalAny failed for spec")
		c.Spec = any
		isc = isSandboxContainer(c)
		assert.Equal(tc.result, isc, "assert failed for checking if is a sandbox container")
	}

}
