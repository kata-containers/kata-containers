// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
package k8s

import (
	"fmt"

	exec "github.com/kata-containers/tests/metrics/exec"
	"github.com/pkg/errors"
)

//logger interface for pkg
var log logger
var Debug bool = false

type logger interface {
	Infof(string, ...interface{})
	Debugf(string, ...interface{})
	Errorf(string, ...interface{})
}

func SetLogger(l logger) {
	log = l
}

type Pod struct {
	YamlPath string
}

func (p *Pod) waitForReady() (err error) {
	log.Debugf("Wait for pod %s", p.YamlPath)
	_, err = exec.ExecCmd("kubectl wait --for=condition=ready -f "+p.YamlPath, Debug)
	return err
}

func (p *Pod) Run() (err error) {

	log.Debugf("Creating K8s Pod %s", p.YamlPath)
	_, err = exec.ExecCmd("kubectl apply -f "+p.YamlPath, Debug)
	if err != nil {
		return errors.Wrapf(err, "Failed to run pod %s", p.YamlPath)
	}

	err = p.waitForReady()
	if err != nil {
		return errors.Wrapf(err, "Failed to wait for pod  %s", p.YamlPath)
	}
	return err
}

func (p *Pod) Delete() (err error) {
	log.Debugf("Delete pod %s", p.YamlPath)
	_, err = exec.ExecCmd("kubectl delete --ignore-not-found -f "+p.YamlPath, Debug)
	return errors.Wrapf(err, "Failed to delete pod %s", p.YamlPath)
}

func (p *Pod) CopyFromHost(src, dst string) (err error) {
	podName, err := exec.ExecCmd("kubectl get -f "+p.YamlPath+" -o jsonpath={.metadata.name}", Debug)
	if err != nil {
		return nil
	}

	log.Debugf("Copy from host %q->%q in pod %s", src, dst, p.YamlPath)
	execCmd := fmt.Sprintf("kubectl cp %s %s:%s", src, podName, dst)
	_, err = exec.ExecCmd(execCmd, Debug)
	return err
}
