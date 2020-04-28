// Copyright 2016 CNI authors
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
	"io/ioutil"
	"os"

	"github.com/containernetworking/cni/pkg/skel"
	"github.com/containernetworking/cni/pkg/types"
	"github.com/containernetworking/cni/pkg/version"
)

func envCleanup() {
	os.Unsetenv("CNI_COMMAND")
	os.Unsetenv("CNI_PATH")
	os.Unsetenv("CNI_NETNS")
	os.Unsetenv("CNI_IFNAME")
	os.Unsetenv("CNI_CONTAINERID")
}

func CmdAdd(cniNetns, cniContainerID, cniIfname string, conf []byte, f func() error) (types.Result, []byte, error) {
	os.Setenv("CNI_COMMAND", "ADD")
	os.Setenv("CNI_PATH", os.Getenv("PATH"))
	os.Setenv("CNI_NETNS", cniNetns)
	os.Setenv("CNI_IFNAME", cniIfname)
	os.Setenv("CNI_CONTAINERID", cniContainerID)
	defer envCleanup()

	// Redirect stdout to capture plugin result
	oldStdout := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		return nil, nil, err
	}

	os.Stdout = w
	err = f()
	w.Close()

	var out []byte
	if err == nil {
		out, err = ioutil.ReadAll(r)
	}
	os.Stdout = oldStdout

	// Return errors after restoring stdout so Ginkgo will correctly
	// emit verbose error information on stdout
	if err != nil {
		return nil, nil, err
	}

	// Plugin must return result in same version as specified in netconf
	versionDecoder := &version.ConfigDecoder{}
	confVersion, err := versionDecoder.Decode(conf)
	if err != nil {
		return nil, nil, err
	}

	result, err := version.NewResult(confVersion, out)
	if err != nil {
		return nil, nil, err
	}

	return result, out, nil
}

func CmdAddWithArgs(args *skel.CmdArgs, f func() error) (types.Result, []byte, error) {
	return CmdAdd(args.Netns, args.ContainerID, args.IfName, args.StdinData, f)
}

func CmdCheck(cniNetns, cniContainerID, cniIfname string, conf []byte, f func() error) error {
	os.Setenv("CNI_COMMAND", "CHECK")
	os.Setenv("CNI_PATH", os.Getenv("PATH"))
	os.Setenv("CNI_NETNS", cniNetns)
	os.Setenv("CNI_IFNAME", cniIfname)
	os.Setenv("CNI_CONTAINERID", cniContainerID)
	defer envCleanup()

	return f()
}

func CmdCheckWithArgs(args *skel.CmdArgs, f func() error) error {
	return CmdCheck(args.Netns, args.ContainerID, args.IfName, args.StdinData, f)
}

func CmdDel(cniNetns, cniContainerID, cniIfname string, f func() error) error {
	os.Setenv("CNI_COMMAND", "DEL")
	os.Setenv("CNI_PATH", os.Getenv("PATH"))
	os.Setenv("CNI_NETNS", cniNetns)
	os.Setenv("CNI_IFNAME", cniIfname)
	os.Setenv("CNI_CONTAINERID", cniContainerID)
	defer envCleanup()

	return f()
}

func CmdDelWithArgs(args *skel.CmdArgs, f func() error) error {
	return CmdDel(args.Netns, args.ContainerID, args.IfName, f)
}
