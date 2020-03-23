// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

const (
	shouldFail    = true
	shouldNotFail = false
)

var _ = SynchronizedBeforeSuite(func() []byte {
	// before start we have to download the docker images
	for _, i := range images {
		// vish/stress is single-arch image only for amd64
		if i == StressImage && runtime.GOARCH != "amd64" {
			//check if vish/stress has already been built
			argsImage := []string{"--format", "'{{.Repository}}:{{.Tag}}'", StressImage}
			imagesStdout, _, imagesExitcode := dockerImages(argsImage...)
			Expect(imagesExitcode).To(BeZero())
			if imagesStdout == "" {
				gopath := os.Getenv("GOPATH")
				entirePath := filepath.Join(gopath, StressDockerFile)
				argsBuild := []string{"-t", StressImage, entirePath}
				_, _, buildExitCode := dockerBuild(argsBuild...)
				Expect(buildExitCode).To(BeZero())
			}
		} else {
			_, _, exitCode := dockerPull(i)
			Expect(exitCode).To(BeZero())
		}
	}

	return nil
}, func(data []byte) {
	// check whether all images were downloaded
	stdout, _, exitCode := dockerImages("--format", "{{.Repository}}")
	Expect(exitCode).To(BeZero())

	dockerImages := strings.Split(stdout, "\n")
	for _, i := range images {
		found := false
		// remove tag
		i = strings.Split(i, ":")[0]
		for _, dockerImage := range dockerImages {
			if i == dockerImage {
				found = true
				break
			}
		}
		Expect(found).To(BeTrue())
	}
})

func TestIntegration(t *testing.T) {
	RegisterFailHandler(Fail)
	RunSpecs(t, "Integration Suite")
}

func TestMain(m *testing.M) {
	tests.KataInit()
	os.Exit(m.Run())
}
