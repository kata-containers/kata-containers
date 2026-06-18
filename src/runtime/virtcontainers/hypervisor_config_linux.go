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

	if conf.RemoteHypervisorSocket != "" {
		return nil
	}

	// IGVM boot supplies the firmware, kernel and measured command line from a
	// single image, so the discrete kernel path is not required in that mode.
	if conf.IgvmPath == "" && conf.KernelPath == "" {
		return fmt.Errorf("Missing kernel path")
	}

	if conf.IgvmPath != "" {
		// The rootfs is still attached as a separate disk, so allow image or
		// initrd (but not both); allow neither for a self-contained IGVM.
		if conf.ImagePath != "" && conf.InitrdPath != "" {
			return fmt.Errorf("Image and initrd path cannot be both set")
		}
	} else if conf.ConfidentialGuest && conf.HypervisorMachineType == QemuCCWVirtio {
		if conf.ImagePath != "" || conf.InitrdPath != "" {
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

	if conf.NumVCPUsF == 0 {
		conf.NumVCPUsF = defaultVCPUs
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

	// CoCo guest hardening: virtio-mmio is not hardened for confidential computing.
	if conf.ConfidentialGuest && conf.BlockDeviceDriver == config.VirtioMmio {
		return fmt.Errorf("confidential guests must not use virtio-mmio (use virtio-blk-pci); virtio-mmio is not hardened for CoCo")
	}

	if conf.DefaultMaxVCPUs == 0 || conf.DefaultMaxVCPUs > defaultMaxVCPUs {
		conf.DefaultMaxVCPUs = defaultMaxVCPUs
	}

	if conf.Msize9p == 0 && conf.SharedFS != config.VirtioFS {
		conf.Msize9p = defaultMsize9p
	}

	return nil
}
