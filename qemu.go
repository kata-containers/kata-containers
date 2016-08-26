/*
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
*/

// Package qemu provides methods and types for launching and managing QEMU
// instances.  Instances can be launched with the LaunchQemu function and
// managed thereafter via QMPStart and the QMP object that this function
// returns.  To manage a qemu instance after it has been launched you need
// to pass the -qmp option during launch requesting the qemu instance to create
// a QMP unix domain manageent socket, e.g.,
// -qmp unix:/tmp/qmp-socket,server,nowait.  For more information see the
// example below.
package qemu

import (
	"bytes"
	"os"
	"os/exec"

	"golang.org/x/net/context"
)

// LaunchQemu can be used to launch a new qemu instance by invoking the
// qemu-system-x86_64 binary.
//
// The ctx parameter is not currently used but has been added so that the
// signature of this function will not need to change when launch cancellation
// is implemented.
//
// params is a slice of options to pass to qemu-system-x86_64 and fds is a
// list of open file descriptors that are to be passed to the spawned qemu
// process.
//
// This function writes its log output via logger parameter.
//
// The function will block until the launched qemu process exits.  "", nil
// will be returned if the launch succeeds.  Otherwise a string containing
// the contents of stderr + a Go error object will be returned.
func LaunchQemu(ctx context.Context, params []string, fds []*os.File, logger QMPLog) (string, error) {
	errStr := ""
	cmd := exec.Command("qemu-system-x86_64", params...)
	if len(fds) > 0 {
		logger.Infof("Adding extra file %v", fds)
		cmd.ExtraFiles = fds
	}

	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	logger.Infof("launching qemu with: %v", params)

	err := cmd.Run()
	if err != nil {
		logger.Errorf("Unable to launch qemu: %v", err)
		errStr = stderr.String()
		logger.Errorf("%s", errStr)
	}
	return errStr, err
}
