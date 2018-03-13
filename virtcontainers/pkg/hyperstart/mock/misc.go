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
