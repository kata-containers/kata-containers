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

// nsenter is a spawner implementation for the nsenter util-linux command.
type nsenter struct {
	ContConfig ContainerConfig
}

const (
	// NsenterCmd is the command used to start nsenter.
	nsenterCmd = "nsenter"
)

// formatArgs is the spawner command formatting implementation for nsenter.
func (n *nsenter) formatArgs(args []string) ([]string, error) {
	var newArgs []string
	pid := "-1"

	// TODO: Retrieve container PID from container ID

	newArgs = append(newArgs, nsenterCmd+" --target "+pid+" --mount --uts --ipc --net --pid")
	newArgs = append(newArgs, args...)

	return newArgs, nil
}
