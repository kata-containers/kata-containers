// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

const (
	acceptEncodingHeader = "Accept-Encoding"
)

// GzipAccepted returns whether the client will accept gzip-encoded content.
func GzipAccepted(header http.Header) bool {
	a := header.Get(acceptEncodingHeader)
	parts := strings.Split(a, ",")
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "gzip" || strings.HasPrefix(part, "gzip;") {
			return true
		}
	}
	return false
}

// String2Pointer make a string to a pointer to string
func String2Pointer(s string) *string {
	return &s
}

// RunCommandFull returns the commands space-trimmed standard output and
// error on success. Note that if the command fails, the requested output will
// still be returned, along with an error.
func RunCommandFull(args []string, includeStderr bool) (string, error) {
	cmd := exec.Command(args[0], args[1:]...)
	var err error
	var bytes []byte

	if includeStderr {
		bytes, err = cmd.CombinedOutput()
	} else {
		bytes, err = cmd.Output()
	}

	trimmed := strings.TrimSpace(string(bytes))

	return trimmed, err
}

// RunCommand returns the commands space-trimmed standard output on success
func RunCommand(args []string) (string, error) {
	return RunCommandFull(args, false)
}

// EnsureDir check if a directory exist, if not then create it
func EnsureDir(path string, mode os.FileMode) error {
	if !filepath.IsAbs(path) {
		return fmt.Errorf("Not an absolute path: %s", path)
	}

	if fi, err := os.Stat(path); err != nil {
		if os.IsNotExist(err) {
			if err = os.MkdirAll(path, mode); err != nil {
				return err
			}
		} else {
			return err
		}
	} else if !fi.IsDir() {
		return fmt.Errorf("Not a directory: %s", path)
	}

	return nil
}
