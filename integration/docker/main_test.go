// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"testing"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

const (
	shouldFail    = true
	shouldNotFail = false
)

func randomDockerName() string {
	return RandID(30)
}

func TestIntegration(t *testing.T) {
	// before start we have to download the docker images
	images := []string{
		Image,
		AlpineImage,
		PostgresImage,
		DebianImage,
		FedoraImage,
		CentosImage,
	}

	for _, i := range images {
		_, _, exitCode := dockerPull(i)
		if exitCode != 0 {
			t.Fatalf("failed to pull docker image: %s\n", i)
		}
	}

	RegisterFailHandler(Fail)
	RunSpecs(t, "Integration Suite")
}
