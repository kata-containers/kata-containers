// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"fmt"
	"math/rand"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/sirupsen/logrus"
)

const (
	acceptEncodingHeader = "Accept-Encoding"
)

var utilsLog = logrus.WithFields(logrus.Fields{"source": "pkg/utils"})

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

func FirstValidExecutable(paths []string) (string, error) {
	for _, p := range paths {
		info, err := os.Stat(p)
		if err != nil {
			if os.IsNotExist(err) {
				continue
			}
			return "", err
		}
		mode := info.Mode()
		// check whether the file is an executable
		if mode&0111 == 0 {
			continue
		}
		return p, nil
	}
	return "", fmt.Errorf("all the executables are invalid")
}

// CreateVmmUser create a temp user for running Kata Containers under rootless mode.
func CreateVmmUser() (string, error) {
	var (
		err      error
		userName string
	)

	useraddPath, err := FirstValidExecutable([]string{"/usr/sbin/useradd", "/sbin/useradd", "/bin/useradd"})
	if err != nil {
		return "", err
	}
	nologinPath, err := FirstValidExecutable([]string{"/usr/sbin/nologin", "/sbin/nologin", "/bin/nologin"})
	if err != nil {
		return "", err
	}

	// Add retries to mitigate temporary errors and race conditions. For example, the user already exists
	// or another instance of the runtime is also creating a user.
	maxAttempt := 5
	for i := 0; i < maxAttempt; i++ {
		userName = fmt.Sprintf("kata-%v", rand.Intn(100000))
		_, err = RunCommand([]string{useraddPath, "-M", "-s", nologinPath, userName, "-c", "\"Kata Containers temporary hypervisor user\""})
		if err == nil {
			return userName, nil
		}
		utilsLog.WithField("attempt", i+1).WithField("username", userName).
			WithError(err).Warn("failed to add user, will try again")
	}
	return "", fmt.Errorf("could not create VMM user: %v", err)
}

// RemoveVmmUser delete user created by CreateVmmUser.
func RemoveVmmUser(user string) error {
	userdelPath, err := FirstValidExecutable([]string{"/usr/sbin/userdel", "/sbin/userdel", "/bin/userdel"})
	if err != nil {
		utilsLog.WithField("username", user).WithError(err).Warn("failed to remove user")
		return err
	}

	// Add retries to mitigate temporary errors and race conditions.
	for i := 0; i < 5; i++ {
		if _, err = RunCommand([]string{userdelPath, "-f", user}); err == nil {
			return nil
		}
		utilsLog.WithField("username", user).WithField("attempt", i+1).WithError(err).Warn("failed to remove user")
	}
	return err
}
