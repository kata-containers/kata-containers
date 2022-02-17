// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
)

func validateHypervisorConfig(conf *HypervisorConfig) error {

	if conf.RemoteHypervisorSocket != "" {
		return nil
	}

	if conf.KernelPath == "" {
		return fmt.Errorf("Missing kernel path")
	}

	if conf.ImagePath == "" && conf.InitrdPath == "" {
		return fmt.Errorf("Missing image and initrd path")
	} else if conf.ImagePath != "" && conf.InitrdPath != "" {
		return fmt.Errorf("Image and initrd path cannot be both set")
	}

	if conf.NumVCPUs == 0 {
		conf.NumVCPUsF = defaultVCPUs
	}

	if conf.MemorySize == 0 {
		conf.MemorySize = defaultMemSzMiB
	}

	return nil
}
