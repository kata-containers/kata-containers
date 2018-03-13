//
// Copyright (c) 2017 Intel Corporation
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
//

package virtcontainers

import (
	"crypto/rand"
	"fmt"
	"os"
	"os/exec"
)

const cpBinaryName = "cp"

const fileMode0755 = os.FileMode(0755)

func fileCopy(srcPath, dstPath string) error {
	if srcPath == "" {
		return fmt.Errorf("Source path cannot be empty")
	}

	if dstPath == "" {
		return fmt.Errorf("Destination path cannot be empty")
	}

	binPath, err := exec.LookPath(cpBinaryName)
	if err != nil {
		return err
	}

	cmd := exec.Command(binPath, srcPath, dstPath)

	return cmd.Run()
}

func generateRandomBytes(n int) ([]byte, error) {
	b := make([]byte, n)
	_, err := rand.Read(b)

	if err != nil {
		return nil, err
	}

	return b, nil
}

func reverseString(s string) string {
	r := []rune(s)

	length := len(r)
	for i, j := 0, length-1; i < length/2; i, j = i+1, j-1 {
		r[i], r[j] = r[j], r[i]
	}

	return string(r)
}

func cleanupFds(fds []*os.File, numFds int) {

	maxFds := len(fds)

	if numFds < maxFds {
		maxFds = numFds
	}

	for i := 0; i < maxFds; i++ {
		_ = fds[i].Close()
	}
}

// writeToFile opens a file in write only mode and writes bytes to it
func writeToFile(path string, data []byte) error {
	f, err := os.OpenFile(path, os.O_WRONLY, fileMode0755)
	if err != nil {
		return err
	}

	defer f.Close()

	if _, err := f.Write(data); err != nil {
		return err
	}

	return nil
}

// ConstraintsToVCPUs converts CPU quota and period to vCPUs
func ConstraintsToVCPUs(quota int64, period uint64) uint {
	if quota != 0 && period != 0 {
		// Use some math magic to round up to the nearest whole vCPU
		// (that is, a partial part of a quota request ends up assigning
		// a whole vCPU, for instance, a request of 1.5 'cpu quotas'
		// will give 2 vCPUs).
		// This also has the side effect that we will always allocate
		// at least 1 vCPU.
		return uint((uint64(quota) + (period - 1)) / period)
	}

	return 0
}
