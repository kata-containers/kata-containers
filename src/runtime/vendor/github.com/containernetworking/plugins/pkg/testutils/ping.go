// Copyright 2017 CNI authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package testutils

import (
	"bytes"
	"fmt"
	"os/exec"
	"strconv"
	"syscall"
)

// Ping shells out to the `ping` command. Returns nil if successful.
func Ping(saddr, daddr string, isV6 bool, timeoutSec int) error {
	args := []string{
		"-c", "1",
		"-W", strconv.Itoa(timeoutSec),
		"-I", saddr,
		daddr,
	}

	bin := "ping"
	if isV6 {
		bin = "ping6"
	}

	cmd := exec.Command(bin, args...)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		switch e := err.(type) {
		case *exec.ExitError:
			return fmt.Errorf("%v exit status %d: %s",
				args, e.Sys().(syscall.WaitStatus).ExitStatus(),
				stderr.String())
		default:
			return err
		}
	}

	return nil
}
