// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package annotations

const (
	vcAnnotationsPrefix = "com.github.containers.virtcontainers."

	// KernelPath is a sandbox annotation for passing a per container path pointing at the kernel needed to boot the container VM.
	KernelPath = vcAnnotationsPrefix + "KernelPath"

	// ImagePath is a sandbox annotation for passing a per container path pointing at the guest image that will run in the container VM.
	ImagePath = vcAnnotationsPrefix + "ImagePath"

	// InitrdPath is a sandbox annotation for passing a per container path pointing at the guest initrd image that will run in the container VM.
	InitrdPath = vcAnnotationsPrefix + "InitrdPath"

	// HypervisorPath is a sandbox annotation for passing a per container path pointing at the hypervisor that will run the container VM.
	HypervisorPath = vcAnnotationsPrefix + "HypervisorPath"

	// FirmwarePath is a sandbox annotation for passing a per container path pointing at the guest firmware that will run the container VM.
	FirmwarePath = vcAnnotationsPrefix + "FirmwarePath"

	// KernelHash is a sandbox annotation for passing a container kernel image SHA-512 hash value.
	KernelHash = vcAnnotationsPrefix + "KernelHash"

	// ImageHash is an sandbox annotation for passing a container guest image SHA-512 hash value.
	ImageHash = vcAnnotationsPrefix + "ImageHash"

	// InitrdHash is an sandbox annotation for passing a container guest initrd SHA-512 hash value.
	InitrdHash = vcAnnotationsPrefix + "InitrdHash"

	// HypervisorHash is an sandbox annotation for passing a container hypervisor binary SHA-512 hash value.
	HypervisorHash = vcAnnotationsPrefix + "HypervisorHash"

	// FirmwareHash is an sandbox annotation for passing a container guest firmware SHA-512 hash value.
	FirmwareHash = vcAnnotationsPrefix + "FirmwareHash"

	// AssetHashType is the hash type used for assets verification
	AssetHashType = vcAnnotationsPrefix + "AssetHashType"

	// ConfigJSONKey is the annotation key to fetch the OCI configuration.
	ConfigJSONKey = vcAnnotationsPrefix + "pkg.oci.config"

	// BundlePathKey is the annotation key to fetch the OCI configuration file path.
	BundlePathKey = vcAnnotationsPrefix + "pkg.oci.bundle_path"

	// ContainerTypeKey is the annotation key to fetch container type.
	ContainerTypeKey = vcAnnotationsPrefix + "pkg.oci.container_type"
)

const (
	// SHA512 is the SHA-512 (64) hash algorithm
	SHA512 string = "sha512"
)
