#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail

script_dir=$(dirname "$0")

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly project="kata-containers"
readonly tmp_dir=$(mktemp -d -t release-notes-tmp.XXXXXXXXXX)

# shellcheck source=../scripts/lib.sh
source "${script_dir}/../scripts/lib.sh"

exit_handler() {
	[ -d "${tmp_dir}" ] || rm -rf "${tmp_dir}"
}
trap exit_handler EXIT

usage() {
	return_code=${1:-}
	cat <<EOT
Usage ${script_name} <previous-release> <new_release>

Args:

previous-release: will be used as start point to get release notes
new-release:      new release version that will have the

Example:
./${script_name} 1.2.0 1.2.1 > notes.md

EOT
	exit "${return_code}"
}

repos=(
	"kata-containers"
)

get_release_info() {

	docker_version=$(get_from_kata_deps "externals.docker.version" "${new_release}")
	crio_version=$(get_from_kata_deps "externals.crio.version")
	cri_containerd_version=$(get_from_kata_deps "externals.cri-containerd.version" "${new_release}")
	kubernetes_version=$(get_from_kata_deps "externals.kubernetes.version" "${new_release}")
	oci_spec_version=$(get_from_kata_deps "specs.oci.version" "${new_release}")

	#Image information
	image_info=$(get_from_kata_deps "assets.image" "${new_release}")

	# Initrd information
	initrd_info=$(get_from_kata_deps "assets.initrd" "${new_release}")

	kernel_version=$(get_from_kata_deps "assets.kernel.version" "${new_release}")
	kernel_url=$(get_from_kata_deps "assets.kernel.url" "${new_release}")

	kata_kernel_config_version="${new_release}-kernel-config"
	kata_kernel_config_version="${new_release}-kernel-config"

	runtime_version=${new_release}
}

changes() {
	echo "**FIXME - message this section by hand to produce a summary please**"

	echo "### Shortlog"
	for cr in $(git log --merges "${previous_release}".."${new_release}" | grep 'Merge:' | awk '{print $2".."$3}'); do
		git log --oneline "$cr"
	done
}

print_release_notes() {
	cat <<EOT
# Release ${runtime_version}

EOT

	for repo in "${repos[@]}"; do
		git clone -q "https://github.com/${project}/${repo}.git" "${tmp_dir}/${repo}"
		pushd "${tmp_dir}/${repo}" >>/dev/null

		cat <<EOT
## ${repo} Changes
$(changes)

EOT
		popd >>/dev/null
		rm -rf "${tmp_dir}/${repo}"
	done

	cat <<EOT

## Compatibility with CRI-O
Kata Containers ${runtime_version} is compatible with CRI-O ${crio_version}

## Compatibility with cri-containerd
Kata Containers ${runtime_version} is compatible with cri-contaienrd ${cri_containerd_version}

## OCI Runtime Specification
Kata Containers ${runtime_version} support the OCI Runtime Specification [${oci_spec_version}][ocispec]

## Compatibility with Kubernetes
Kata Containers ${runtime_version} is compatible with Kubernetes ${kubernetes_version}

## Kata Linux Containers image
Agent version: ${new_release}

### Default Image Guest OS:
${image_info}

### Default Initrd Guest OS:
${initrd_info}

## Kata Linux Containers Kernel
Kata Containers ${runtime_version} suggest to use the Linux kernel [${kernel_version}][kernel]
See the kernel suggested [Guest Kernel patches][kernel-patches]
See the kernel suggested [Guest Kernel config][kernel-config]

## Installation

Follow the Kata [installation instructions][installation].

## Issues & limitations

More information [Limitations][limitations]

[kernel]: ${kernel_url}/linux-${kernel_version#v}.tar.xz
[kernel-patches]: https://github.com/kata-containers/kata-containers/tree/${new_release}/tools/packaging/kernel/patches
[kernel-config]: https://github.com/kata-containers/kata-containers/tree/${new_release}/tools/packaging/kernel/configs
[ocispec]: https://github.com/opencontainers/runtime-spec/releases/tag/${oci_spec_version}
[limitations]: https://github.com/kata-containers/kata-containers/blob/${new_release}/docs/Limitations.md
[installation]: https://github.com/kata-containers/kata-containers/blob/${new_release}/docs/install
EOT
}

main() {
	previous_release=${1:-}
	new_release=${2:-}
	if [ -z "${previous_release}" ]; then
		echo "previous-release not provided"
		usage 1
	fi
	if [ -z "${new_release}" ]; then
		echo "new-release not provided"
		usage 1
	fi
	get_release_info
	print_release_notes
}

main "$@"
