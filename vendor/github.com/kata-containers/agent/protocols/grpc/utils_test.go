//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package grpc

import (
	"encoding/json"
	"io/ioutil"
	"reflect"
	"testing"

	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

const ociConfigFile = "config.json"

func assertIsEqual(t *testing.T, ociSpec *specs.Spec, grpcSpec *Spec) {
	assert := assert.New(t)

	// Version check
	assert.Equal(grpcSpec.Version, ociSpec.Version)

	// Process checks: User
	assert.Equal(grpcSpec.Process.User.UID, ociSpec.Process.User.UID)
	assert.Equal(grpcSpec.Process.User.GID, ociSpec.Process.User.GID)

	// Process checks: Capabilities
	assert.Equal(grpcSpec.Process.Capabilities.Bounding, ociSpec.Process.Capabilities.Bounding)
	assert.Equal(grpcSpec.Process.Capabilities.Effective, ociSpec.Process.Capabilities.Effective)
	assert.Equal(grpcSpec.Process.Capabilities.Inheritable, ociSpec.Process.Capabilities.Inheritable)
	assert.Equal(grpcSpec.Process.Capabilities.Permitted, ociSpec.Process.Capabilities.Permitted)
	assert.Equal(grpcSpec.Process.Capabilities.Ambient, ociSpec.Process.Capabilities.Ambient)

	// Annotations checks: Annotations
	assert.Equal(len(grpcSpec.Annotations), len(ociSpec.Annotations))

	for k := range grpcSpec.Annotations {
		assert.Equal(grpcSpec.Annotations[k], ociSpec.Annotations[k])
	}

	// Linux checks: Devices
	assert.Equal(len(grpcSpec.Linux.Resources.Devices), len(ociSpec.Linux.Resources.Devices))
	assert.Equal(len(grpcSpec.Linux.Resources.Devices), 1)
	assert.Equal(grpcSpec.Linux.Resources.Devices[0].Access, "rwm")

	// Linux checks: Block IO, for checking embedded structures copy
	assert.NotNil(ociSpec.Linux.Resources.BlockIO.LeafWeight)
	assert.NotNil(ociSpec.Linux.Resources.BlockIO.Weight)
	assert.EqualValues(grpcSpec.Linux.Resources.BlockIO.Weight, *ociSpec.Linux.Resources.BlockIO.Weight)
	assert.EqualValues(grpcSpec.Linux.Resources.BlockIO.LeafWeight, *ociSpec.Linux.Resources.BlockIO.LeafWeight)
	assert.NotEqual(len(grpcSpec.Linux.Resources.BlockIO.WeightDevice), 0)
	assert.Equal(len(grpcSpec.Linux.Resources.BlockIO.WeightDevice), len(grpcSpec.Linux.Resources.BlockIO.WeightDevice))
	assert.EqualValues(grpcSpec.Linux.Resources.BlockIO.WeightDevice[0].Major, ociSpec.Linux.Resources.BlockIO.WeightDevice[0].Major)
	assert.EqualValues(grpcSpec.Linux.Resources.BlockIO.WeightDevice[0].Minor, ociSpec.Linux.Resources.BlockIO.WeightDevice[0].Minor)
	assert.NotNil(ociSpec.Linux.Resources.BlockIO.WeightDevice[0].LeafWeight)
	assert.NotNil(ociSpec.Linux.Resources.BlockIO.WeightDevice[0].Weight)
	assert.EqualValues(grpcSpec.Linux.Resources.BlockIO.WeightDevice[0].Weight, *ociSpec.Linux.Resources.BlockIO.WeightDevice[0].Weight)
	assert.EqualValues(grpcSpec.Linux.Resources.BlockIO.WeightDevice[0].LeafWeight, *ociSpec.Linux.Resources.BlockIO.WeightDevice[0].LeafWeight)

	// Linux checks: Namespaces
	assert.Equal(len(grpcSpec.Linux.Namespaces), len(ociSpec.Linux.Namespaces))
	assert.Equal(len(grpcSpec.Linux.Namespaces), 5)

	for i := range grpcSpec.Linux.Namespaces {
		assert.Equal(grpcSpec.Linux.Namespaces[i].Type, (string)(ociSpec.Linux.Namespaces[i].Type))
		assert.Equal(grpcSpec.Linux.Namespaces[i].Path, (string)(ociSpec.Linux.Namespaces[i].Path))
	}
}

func TestOCItoGRPC(t *testing.T) {
	assert := assert.New(t)
	var ociSpec specs.Spec

	configJsonBytes, err := ioutil.ReadFile(ociConfigFile)
	assert.NoError(err, "Could not open OCI config file")

	err = json.Unmarshal(configJsonBytes, &ociSpec)
	assert.NoError(err, "Could not unmarshall OCI config file")

	spec, err := OCItoGRPC(&ociSpec)
	assert.NoError(err, "Could not convert OCI config file")
	assertIsEqual(t, &ociSpec, spec)
}

func TestGRPCtoOCI(t *testing.T) {
	assert := assert.New(t)

	var ociSpec specs.Spec

	configJsonBytes, err := ioutil.ReadFile(ociConfigFile)
	assert.NoError(err, "Could not open OCI config file")

	err = json.Unmarshal(configJsonBytes, &ociSpec)
	assert.NoError(err, "Could not unmarshall OCI config file")

	grpcSpec, err := OCItoGRPC(&ociSpec)
	assert.NoError(err, "Could not convert OCI config file")

	newOciSpec, err := GRPCtoOCI(grpcSpec)
	assert.NoError(err, "Could not convert gRPC structure")

	assertIsEqual(t, newOciSpec, grpcSpec)
}

func testCopyValue(t *testing.T, to, from interface{}) {
	assert := assert.New(t)

	err := copyValue(reflect.ValueOf(to).Elem(), reflect.ValueOf(from))
	assert.NoError(err, "Could not copy to %v", reflect.ValueOf(from).Kind())
	assert.Equal(reflect.ValueOf(to).Elem().Interface(), reflect.ValueOf(from).Interface())
}

func TestCopyValueString(t *testing.T) {
	from := "foobar"
	to := new(string)

	testCopyValue(t, to, from)
}

func TestCopyValueSlice(t *testing.T) {
	from := []string{"foobar", "barfoo"}
	to := new([]string)

	testCopyValue(t, to, from)
}

func TestCopyValueStruc(t *testing.T) {
	type dummyStruct struct {
		S string
		I int
	}

	from := dummyStruct{
		S: "foobar",
		I: 18,
	}
	to := new(dummyStruct)

	testCopyValue(t, to, from)
}

func TestCopyValueMap(t *testing.T) {
	from := map[string]string{
		"key1": "value1",
		"key2": "value2",
	}
	to := new(map[string]string)

	testCopyValue(t, to, from)
}
