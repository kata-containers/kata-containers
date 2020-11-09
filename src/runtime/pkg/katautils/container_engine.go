// Copyright (c) 2019 SUSE LLC
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"os/exec"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
)

type CtrEngine struct {
	Name string
}

var (
	DockerLikeCtrEngines = []string{"docker", "podman"}
)

func (e *CtrEngine) Init(name string) (string, error) {
	var out string
	out, err := utils.RunCommandFull([]string{name, "version"}, true)
	if err != nil {
		return out, err
	}

	e.Name = name
	return out, nil
}

func (e *CtrEngine) Inspect(image string) (string, error) {
	// Only hit the network if the image doesn't exist locally
	return utils.RunCommand([]string{e.Name, "inspect", "--type=image", image})
}

func (e *CtrEngine) Pull(image string) (string, error) {
	return utils.RunCommand([]string{e.Name, "pull", image})
}

func (e *CtrEngine) Create(image string) (string, error) {
	return utils.RunCommand([]string{e.Name, "create", image})
}

func (e *CtrEngine) Rm(ctrID string) (string, error) {
	return utils.RunCommand([]string{e.Name, "rm", ctrID})
}

func (e *CtrEngine) GetRootfs(ctrID string, dir string) error {
	cmd1 := exec.Command(e.Name, "export", ctrID)
	cmd2 := exec.Command("tar", "-C", dir, "-xvf", "-")

	cmd1Stdout, err := cmd1.StdoutPipe()
	if err != nil {
		return err
	}

	cmd2.Stdin = cmd1Stdout

	err = cmd2.Start()
	if err != nil {
		return err
	}

	err = cmd1.Run()
	if err != nil {
		return err
	}

	err = cmd2.Wait()
	if err != nil {
		return err
	}

	return nil
}
