// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"fmt"
	"os"
	"path"
	"testing"

	"github.com/stretchr/testify/assert"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
	"github.com/pkg/errors"
)

const (
	TestID = "container_test"

	testFileMode = os.FileMode(0640)

	testSandboxID   = "777-77-77777777"
	testContainerID = "42"

	testContainerTypeAnnotation = "io.kubernetes.cri.container-type"
	testSandboxIDAnnotation     = "io.kubernetes.cri.sandbox-id"
	testContainerTypeSandbox    = "sandbox"
	testContainerTypeContainer  = "container"
)

var (
	// package variables set by calling TestMain()
	tc ktu.TestConstraint
)

// testingImpl is a concrete mock RVC implementation used for testing
var testingImpl = &vcmock.VCMock{}

func init() {
	fmt.Printf("INFO: running as actual user %v (effective %v), actual group %v (effective %v)\n",
		os.Getuid(), os.Geteuid(), os.Getgid(), os.Getegid())

	fmt.Printf("INFO: switching to fake virtcontainers implementation for testing\n")
	vci = testingImpl

	tc = ktu.NewTestConstraint(false)

	// disable shim management server.
	// all tests are not using this, so just set it to nil
	defaultStartManagementServerFunc = nil
}

func createEmptyFile(path string) (err error) {
	return os.WriteFile(path, []byte(""), testFileMode)
}

// newTestHypervisorConfig creaets a new virtcontainers
// HypervisorConfig, ensuring that the required resources are also
// created.
//
// Note: no parameter validation in case caller wishes to create an invalid
// object.
func newTestHypervisorConfig(dir string, create bool) (vc.HypervisorConfig, error) {
	kernelPath := path.Join(dir, "kernel")
	imagePath := path.Join(dir, "image")
	hypervisorPath := path.Join(dir, "hypervisor")

	if create {
		for _, file := range []string{kernelPath, imagePath, hypervisorPath} {
			err := createEmptyFile(file)
			if err != nil {
				return vc.HypervisorConfig{}, err
			}
		}
	}

	return vc.HypervisorConfig{
		KernelPath:            kernelPath,
		ImagePath:             imagePath,
		HypervisorPath:        hypervisorPath,
		HypervisorMachineType: "q35",
	}, nil
}

// newTestRuntimeConfig creates a new RuntimeConfig
func newTestRuntimeConfig(dir string, create bool) (oci.RuntimeConfig, error) {
	if dir == "" {
		return oci.RuntimeConfig{}, errors.New("BUG: need directory")
	}

	hypervisorConfig, err := newTestHypervisorConfig(dir, create)
	if err != nil {
		return oci.RuntimeConfig{}, err
	}

	return oci.RuntimeConfig{
		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,
	}, nil
}

func TestNoNeedForOutput(t *testing.T) {
	assert := assert.New(t)

	testCases := []struct {
		detach bool
		tty    bool
		result bool
	}{
		{
			detach: true,
			tty:    true,
			result: true,
		},
		{
			detach: false,
			tty:    true,
			result: false,
		},
		{
			detach: true,
			tty:    false,
			result: false,
		},
		{
			detach: false,
			tty:    false,
			result: false,
		},
	}

	for i := range testCases {
		result := noNeedForOutput(testCases[i].detach, testCases[i].tty)
		assert.Equal(testCases[i].result, result)
	}
}
