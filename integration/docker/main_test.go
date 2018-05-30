// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"os"
	"strings"
	"testing"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

const (
	shouldFail    = true
	shouldNotFail = false
)

var runtimeConfig RuntimeConfig

func randomDockerName() string {
	return RandID(30)
}

func TestIntegration(t *testing.T) {
	var err error
	runtimeConfigPath := DefaultRuntimeConfigPath

	args := []string{"--kata-show-default-config-paths"}
	cmd := NewCommand(Runtime, args...)
	stdout, _, exitCode := cmd.Run()
	if exitCode == 0 && stdout != "" {
		for _, c := range strings.Split(stdout, "\n") {
			if _, err = os.Stat(c); err == nil {
				runtimeConfigPath = c
				break
			}
		}
	}

	runtimeConfig, err = LoadRuntimeConfiguration(runtimeConfigPath)
	if err != nil {
		t.Fatalf("failed to load runtime configuration: %v\n", err)
	}

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
