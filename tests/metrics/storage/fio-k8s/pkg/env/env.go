// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
package env

import (
	exec "github.com/kata-containers/tests/metrics/exec"
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

var sysDropCachesPath = "/proc/sys/vm/drop_caches"

func DropCaches() (err error) {
	log.Infof("drop caches")
	_, err = exec.ExecCmd("sync", Debug)
	if err != nil {
		return err
	}

	_, err = exec.ExecCmd("echo 3 | sudo tee "+sysDropCachesPath, Debug)
	if err != nil {
		return err
	}
	return nil
}
