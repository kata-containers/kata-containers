// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package tests

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	spec "github.com/opencontainers/specs/specs-go"
)

const image = "busybox"

const tmpDir = "/tmp"

const configTemplate = "src/github.com/kata-containers/tests/data/config.json"

// Bundle represents the root directory where config.json and rootfs are
type Bundle struct {
	// Config represents the config.json
	Config *spec.Spec

	// Path to the bundle
	Path string
}

// NewBundle creates a new bundle
func NewBundle(workload []string) (*Bundle, error) {
	path, err := ioutil.TempDir("", "bundle")
	if err != nil {
		return nil, err
	}

	if err := createRootfs(path); err != nil {
		return nil, err
	}

	gopath := os.Getenv("GOPATH")
	if gopath == "" {
		return nil, fmt.Errorf("GOPATH is not set")
	}

	configTemplatePath := filepath.Join(gopath, configTemplate)
	content, err := ioutil.ReadFile(configTemplatePath)
	if err != nil {
		return nil, err
	}

	var config spec.Spec
	err = json.Unmarshal(content, &config)
	if err != nil {
		return nil, err
	}

	// By default, let's not create a terminal
	config.Process.Terminal = false

	config.Process.Args = workload

	bundle := &Bundle{
		Path:   path,
		Config: &config,
	}

	err = bundle.Save()
	if err != nil {
		return nil, err
	}

	return bundle, nil
}

// createRootfs creates a rootfs in the specific bundlePath
func createRootfs(bundlePath string) error {
	if bundlePath == "" {
		return fmt.Errorf("bundle path should not be empty")
	}

	rootfsDir := filepath.Join(bundlePath, "rootfs")
	if err := os.MkdirAll(rootfsDir, 0755); err != nil {
		return err
	}

	// create container
	var container bytes.Buffer
	createCmd := exec.Command("docker", "create", image)
	createCmd.Stdout = &container
	if err := createCmd.Run(); err != nil {
		return err
	}
	containerName := strings.TrimRight(container.String(), "\n")

	// export container
	tarFile, err := ioutil.TempFile(tmpDir, "tar")
	if err != nil {
		return err
	}
	defer tarFile.Close()

	exportCmd := exec.Command("docker", "export", "-o", tarFile.Name(), containerName)
	if err := exportCmd.Run(); err != nil {
		return err
	}
	defer os.Remove(tarFile.Name())

	// extract container
	tarCmd := exec.Command("tar", "-C", rootfsDir, "-pxf", tarFile.Name())
	if err := tarCmd.Run(); err != nil {
		return err
	}

	// remove container
	rmCmd := exec.Command("docker", "rm", "-f", containerName)
	return rmCmd.Run()
}

// Save to disk the Config
func (b *Bundle) Save() error {
	content, err := json.Marshal(b.Config)
	if err != nil {
		return err
	}

	configFile := filepath.Join(b.Path, "config.json")
	err = ioutil.WriteFile(configFile, content, 0644)
	if err != nil {
		return err
	}

	return nil
}

// Remove the bundle files and directories
func (b *Bundle) Remove() error {
	return os.RemoveAll(b.Path)
}
