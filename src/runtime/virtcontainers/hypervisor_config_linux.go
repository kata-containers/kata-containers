// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
)

func validateHypervisorConfig(conf *HypervisorConfig) error {

	if conf.KernelPath == "" {
		return fmt.Errorf("Missing kernel path")
	}

	if conf.ConfidentialGuest && conf.HypervisorMachineType == QemuCCWVirtio {
		if conf.ImagePath != "" || conf.InitrdPath != "" {
			fmt.Println("yes, failing")
			return fmt.Errorf("Neither the image or initrd path may be set for Secure Execution")
		}
	} else if conf.ImagePath == "" && conf.InitrdPath == "" {
		return fmt.Errorf("Missing image and initrd path")
	} else if conf.ImagePath != "" && conf.InitrdPath != "" {
		return fmt.Errorf("Image and initrd path cannot be both set")
	}

	if err := conf.CheckTemplateConfig(); err != nil {
		return err
	}

	if conf.NumVCPUs == 0 {
		conf.NumVCPUs = defaultVCPUs
	}

	if conf.MemorySize == 0 {
		conf.MemorySize = defaultMemSzMiB
	}

	if conf.DefaultBridges == 0 {
		conf.DefaultBridges = defaultBridges
	}

	if conf.BlockDeviceDriver == "" {
		conf.BlockDeviceDriver = defaultBlockDriver
	} else if conf.BlockDeviceDriver == config.VirtioBlock && conf.HypervisorMachineType == QemuCCWVirtio {
		conf.BlockDeviceDriver = config.VirtioBlockCCW
	}

	if conf.DefaultMaxVCPUs == 0 || conf.DefaultMaxVCPUs > defaultMaxVCPUs {
		conf.DefaultMaxVCPUs = defaultMaxVCPUs
	}

	if conf.ConfidentialGuest && conf.NumVCPUs != conf.DefaultMaxVCPUs {
		hvLogger.Warnf("Confidential guests do not support hotplugging of vCPUs. Setting DefaultMaxVCPUs to NumVCPUs (%d)", conf.NumVCPUs)
		conf.DefaultMaxVCPUs = conf.NumVCPUs
	}

	if conf.Msize9p == 0 && conf.SharedFS != config.VirtioFS {
		conf.Msize9p = defaultMsize9p
	}

	return nil
}
