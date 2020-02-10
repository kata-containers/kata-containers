// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package podman

import (
	"testing"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

func TestIntegration(t *testing.T) {
	// before start we have to download the docker images
	images := []string{
		Image,
	}

	for _, i := range images {
		_, _, exitCode := podmanPull(i)
		if exitCode != 0 {
			t.Fatalf("failed to pull podman image: %s\n", i)
		}
	}

	RegisterFailHandler(Fail)
	RunSpecs(t, "Integration Suite")
}
