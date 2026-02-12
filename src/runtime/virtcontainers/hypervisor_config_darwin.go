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
		return fmt.Errorf("missing kernel path")
	}

	if conf.ImagePath == "" && conf.InitrdPath == "" {
		return fmt.Errorf("missing image and initrd path")
	} else if conf.ImagePath != "" && conf.InitrdPath != "" {
		return fmt.Errorf("image and initrd path cannot be both set")
	}

	if conf.NumVCPUs == 0 {
		conf.NumVCPUsF = defaultVCPUs
	}

	if conf.MemorySize == 0 {
		conf.MemorySize = defaultMemSzMiB
	}

	return nil
}
