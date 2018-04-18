// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package mock

import (
	"fmt"
	"os"
	"path/filepath"
)

// GetTmpPath will return a filename suitable for a tempory file according to
// the format string given in argument. The format string must contain a single
// %s which will be replaced by a random string. Eg.:
//
//   GetTmpPath("test.foo.%s.sock")
//
// will return something like:
//
//   "/tmp/test.foo.832222621.sock"
func GetTmpPath(format string) string {
	filename := fmt.Sprintf(format, nextSuffix())
	dir := os.TempDir()
	return filepath.Join(dir, filename)

}
