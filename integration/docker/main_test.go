// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"os"
	"path/filepath"
	"runtime"
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
		StressImage,
	}

	for _, i := range images {
		// vish/stress is single-arch image only for amd64
		if i == StressImage && runtime.GOARCH == "arm64" {
			//check if vish/stress has already been built
			argsImage := []string{"--format", "'{{.Repository}}:{{.Tag}}'", StressImage}
			imagesStdout, _, imagesExitcode := dockerImages(argsImage...)
			if imagesExitcode != 0 {
				t.Fatalf("failed to docker images --format '{{.Repository}}:{{.Tag}}' %s\n", StressImage)
			}
			if imagesStdout == "" {
				gopath := os.Getenv("GOPATH")
				entirePath := filepath.Join(gopath, StressDockerFile)
				argsBuild := []string{"-t", StressImage, entirePath}
				_, _, buildExitCode := dockerBuild(argsBuild...)
				if buildExitCode != 0 {
					t.Fatalf("failed to build stress image in %s\n", runtime.GOARCH)
				}
			}
		} else {
			_, _, exitCode := dockerPull(i)
			if exitCode != 0 {
				t.Fatalf("failed to pull docker image: %s\n", i)
			}
		}
	}

	RegisterFailHandler(Fail)
	RunSpecs(t, "Integration Suite")
}
