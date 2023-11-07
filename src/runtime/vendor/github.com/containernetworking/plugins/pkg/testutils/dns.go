// Copyright 2019 CNI authors
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
	"fmt"
	"os"
	"strings"

	"github.com/containernetworking/cni/pkg/types"
)

// TmpResolvConf will create a temporary file and write the provided DNS settings to
// it in the resolv.conf format. It returns the path of the created temporary file or
// an error if any occurs while creating/writing the file. It is the caller's
// responsibility to remove the file.
func TmpResolvConf(dnsConf types.DNS) (string, error) {
	f, err := os.CreateTemp("", "cni_test_resolv.conf")
	if err != nil {
		return "", fmt.Errorf("failed to get temp file for CNI test resolv.conf: %v", err)
	}
	defer f.Close()

	path := f.Name()
	defer func() {
		if err != nil {
			os.RemoveAll(path)
		}
	}()

	// see "man 5 resolv.conf" for the format of resolv.conf
	var resolvConfLines []string
	for _, nameserver := range dnsConf.Nameservers {
		resolvConfLines = append(resolvConfLines, fmt.Sprintf("nameserver %s", nameserver))
	}
	resolvConfLines = append(resolvConfLines, fmt.Sprintf("domain %s", dnsConf.Domain))
	resolvConfLines = append(resolvConfLines, fmt.Sprintf("search %s", strings.Join(dnsConf.Search, " ")))
	resolvConfLines = append(resolvConfLines, fmt.Sprintf("options %s", strings.Join(dnsConf.Options, " ")))

	resolvConf := strings.Join(resolvConfLines, "\n")
	_, err = f.Write([]byte(resolvConf))
	if err != nil {
		return "", fmt.Errorf("failed to write temp resolv.conf for CNI test: %v", err)
	}

	return path, err
}
