// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
package exec

import (
	"bytes"
	"io"
	"os"
	"os/exec"

	"github.com/pkg/errors"
)

//logger interface for pkg
var log logger

type logger interface {
	Infof(string, ...interface{})
	Debugf(string, ...interface{})
	Errorf(string, ...interface{})
}

func SetLogger(l logger) {
	log = l
}

// Exec a command
// err != nil if command fails to execute
// output is a string with a combined stdout and stderr
func ExecCmd(c string, showInStdout bool) (stdout string, err error) {
	if c == "" {
		return "", errors.New("command is empty")
	}

	log.Debugf("Exec: %s", c)
	cmd := exec.Command("bash", "-o", "pipefail", "-c", c)
	var stdBuffer bytes.Buffer
	var writers []io.Writer
	writers = append(writers, &stdBuffer)
	if showInStdout {
		writers = append(writers, os.Stdout)
	}
	mw := io.MultiWriter(writers...)

	cmd.Stdout = mw
	cmd.Stderr = mw

	err = cmd.Run()
	output := stdBuffer.String()

	return stdBuffer.String(), errors.Wrap(err, output)
}

// Exec a command
// Send output to Stdout and Stderr
func ExecStdout(c string) error {
	if c == "" {
		return errors.New("command is empty")
	}

	log.Debugf("Exec: %s", c)
	cmd := exec.Command("bash", "-o", "pipefail", "-c", c)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}
