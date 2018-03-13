//
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
//

package virtcontainers

import (
	"strings"
	"testing"
)

func testNsEnterFormatArgs(t *testing.T, args []string, expected string) {
	nsenter := &nsenter{}

	cmd, err := nsenter.formatArgs(args)
	if err != nil {
		t.Fatal(err)
	}

	if strings.Join(cmd, " ") != expected {
		t.Fatal()
	}
}

func TestNsEnterFormatArgsHello(t *testing.T) {
	expectedCmd := "nsenter --target -1 --mount --uts --ipc --net --pid echo hello"

	args := []string{"echo", "hello"}

	testNsEnterFormatArgs(t, args, expectedCmd)
}
