//
// Copyright (c) 2019 Intel Corporation
// Copyright (c) 2019 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package main

const (
	rootBusPath = "/devices/pci0000:00"

	// From https://www.kernel.org/doc/Documentation/acpi/namespace.txt
	// The Linux kernel's core ACPI subsystem creates struct acpi_device
	// objects for ACPI namespace objects representing devices, power resources
	// processors, thermal zones. Those objects are exported to user space via
	// sysfs as directories in the subtree under /sys/devices/LNXSYSTM:00
	acpiDevPath = "/devices/LNXSYSTM"

	// /dev/pmemX devices exported in the ACPI sysfs (/devices/LNXSYSTM) are
	// in a subdirectory whose prefix is pfn (page frame number).
	pfnDevPrefix = "/pfn"
)
