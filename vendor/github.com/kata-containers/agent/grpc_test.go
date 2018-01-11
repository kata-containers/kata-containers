//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"reflect"
	"testing"

	"github.com/opencontainers/runc/libcontainer/configs"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

var testSharedPidNs = "testSharedPidNs"

func testUpdateContainerConfigNamespaces(t *testing.T, sharedPidNs string, config, expected configs.Config) {
	a := &agentGRPC{
		sandbox: &sandbox{
			sharedPidNs: namespace{
				path: sharedPidNs,
			},
		},
	}

	err := a.updateContainerConfigNamespaces(&config)
	assert.Nil(t, err, "updateContainerConfigNamespaces() failed: %v", err)

	assert.True(t, reflect.DeepEqual(config, expected),
		"Config structures should be identical: got %+v, expecting %+v",
		config, expected)
}

func TestUpdateContainerConfigNamespacesNonEmptyConfig(t *testing.T) {
	config := configs.Config{
		Namespaces: []configs.Namespace{
			{
				Type: configs.NEWPID,
			},
		},
	}

	expectedConfig := configs.Config{
		Namespaces: []configs.Namespace{
			{
				Type: configs.NEWPID,
				Path: testSharedPidNs,
			},
		},
	}

	testUpdateContainerConfigNamespaces(t, testSharedPidNs, config, expectedConfig)
}

func TestUpdateContainerConfigNamespacesEmptyConfig(t *testing.T) {
	expectedConfig := configs.Config{
		Namespaces: []configs.Namespace{
			{
				Type: configs.NEWPID,
				Path: testSharedPidNs,
			},
		},
	}

	testUpdateContainerConfigNamespaces(t, testSharedPidNs, configs.Config{}, expectedConfig)
}

func testUpdateContainerConfigPrivileges(t *testing.T, spec *specs.Spec, config, expected configs.Config) {
	a := &agentGRPC{}

	err := a.updateContainerConfigPrivileges(spec, &config)
	assert.Nil(t, err, "updateContainerConfigPrivileges() failed: %v", err)

	assert.True(t, reflect.DeepEqual(config, expected),
		"Config structures should be identical: got %+v, expecting %+v",
		config, expected)
}

func TestUpdateContainerConfigPrivilegesNilSpec(t *testing.T) {
	testUpdateContainerConfigPrivileges(t, nil, configs.Config{}, configs.Config{})
}

func TestUpdateContainerConfigPrivilegesNilSpecProcess(t *testing.T) {
	testUpdateContainerConfigPrivileges(t, &specs.Spec{}, configs.Config{}, configs.Config{})
}

func TestUpdateContainerConfigPrivilegesNoNewPrivileges(t *testing.T) {
	for _, priv := range []bool{false, true} {
		spec := &specs.Spec{
			Process: &specs.Process{
				NoNewPrivileges: priv,
			},
		}
		config := configs.Config{}
		expectedConfig := configs.Config{
			NoNewPrivileges: priv,
		}

		testUpdateContainerConfigPrivileges(t, spec, config, expectedConfig)
	}
}
