// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package annotations

const (
	kataAnnotationsPrefix     = "io.kata-containers."
	kataConfAnnotationsPrefix = kataAnnotationsPrefix + "config."
	kataAnnotHypervisorPrefix = kataConfAnnotationsPrefix + "hypervisor."
	kataAnnotAgentPrefix      = kataConfAnnotationsPrefix + "agent."
	kataAnnotRuntimePrefix    = kataConfAnnotationsPrefix + "runtime." // nolint: unused

	// KernelPath is a sandbox annotation for passing a per container path pointing at the kernel needed to boot the container VM.
	KernelPath = kataAnnotHypervisorPrefix + "kernel"

	// ImagePath is a sandbox annotation for passing a per container path pointing at the guest image that will run in the container VM.
	ImagePath = kataAnnotHypervisorPrefix + "image"

	// InitrdPath is a sandbox annotation for passing a per container path pointing at the guest initrd image that will run in the container VM.
	InitrdPath = kataAnnotHypervisorPrefix + "initrd"

	// HypervisorPath is a sandbox annotation for passing a per container path pointing at the hypervisor that will run the container VM.
	HypervisorPath = kataAnnotHypervisorPrefix + "path"

	// JailerPath is a sandbox annotation for passing a per container path pointing at the jailer that will constrain the container VM.
	JailerPath = kataAnnotHypervisorPrefix + "jailer_path"

	// FirmwarePath is a sandbox annotation for passing a per container path pointing at the guest firmware that will run the container VM.
	FirmwarePath = kataAnnotHypervisorPrefix + "firmware"

	// KernelHash is a sandbox annotation for passing a container kernel image SHA-512 hash value.
	KernelHash = kataAnnotHypervisorPrefix + "kernel_hash"

	// ImageHash is an sandbox annotation for passing a container guest image SHA-512 hash value.
	ImageHash = kataAnnotHypervisorPrefix + "image_hash"

	// InitrdHash is an sandbox annotation for passing a container guest initrd SHA-512 hash value.
	InitrdHash = kataAnnotHypervisorPrefix + "initrd_hash"

	// HypervisorHash is an sandbox annotation for passing a container hypervisor binary SHA-512 hash value.
	HypervisorHash = kataAnnotHypervisorPrefix + "hypervisor_hash"

	// JailerHash is an sandbox annotation for passing a jailer binary SHA-512 hash value.
	JailerHash = kataAnnotHypervisorPrefix + "jailer_hash"

	// FirmwareHash is an sandbox annotation for passing a container guest firmware SHA-512 hash value.
	FirmwareHash = kataAnnotHypervisorPrefix + "firmware_hash"

	// AssetHashType is the hash type used for assets verification
	AssetHashType = kataAnnotationsPrefix + "asset_hash_type"

	// BundlePathKey is the annotation key to fetch the OCI configuration file path.
	BundlePathKey = kataAnnotationsPrefix + "pkg.oci.bundle_path"

	// ContainerTypeKey is the annotation key to fetch container type.
	ContainerTypeKey = kataAnnotationsPrefix + "pkg.oci.container_type"

	// KernelModules is the annotation key for passing the list of kernel
	// modules and their parameters that will be loaded in the guest kernel.
	// Semicolon separated list of kernel modules and their parameters.
	// These modules will be loaded in the guest kernel using modprobe(8).
	// The following example can be used to load two kernel modules with parameters
	///
	//   annotations:
	//     io.kata-containers.config.agent.kernel_modules: "e1000e InterruptThrottleRate=3000,3000,3000 EEE=1; i915 enable_ppgtt=0"
	//
	// The first word is considered as the module name and the rest as its parameters.
	//
	KernelModules = kataAnnotAgentPrefix + "kernel_modules"
)

const (
	// SHA512 is the SHA-512 (64) hash algorithm
	SHA512 string = "sha512"
)
