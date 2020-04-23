// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
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
