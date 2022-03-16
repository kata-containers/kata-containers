#!/usr/bin/env bash
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
	cat <<EOF
Usage ${script_name} <previous-release> <new_release>

Args:

previous-release: will be used as start point to get release notes
new-release:      new release version that will have the

Example:
./${script_name} 1.2.0 1.2.1 > notes.md

EOF
	exit "${return_code}"
}

repos=(
	"kata-containers"
)

get_release_info() {

	docker_version=$(get_from_kata_deps "externals.docker.version")
	crio_version=$(get_from_kata_deps "externals.crio.version")
	containerd_version=$(get_from_kata_deps "externals.containerd.version")
	kubernetes_version=$(get_from_kata_deps "externals.kubernetes.version")
	oci_spec_version=$(get_from_kata_deps "specs.oci.version")

	libseccomp_version=$(get_from_kata_deps "externals.libseccomp.version")
	libseccomp_url=$(get_from_kata_deps "externals.libseccomp.url")

	#Image information
	image_info=$(get_from_kata_deps "assets.image")

	# Initrd information
	initrd_info=$(get_from_kata_deps "assets.initrd")

	kernel_version=$(get_from_kata_deps "assets.kernel.version")
	kernel_url=$(get_from_kata_deps "assets.kernel.url")

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
	cat <<EOF
# Release ${runtime_version}

EOF

	for repo in "${repos[@]}"; do
		git clone -q "https://github.com/${project}/${repo}.git" "${tmp_dir}/${repo}"
		pushd "${tmp_dir}/${repo}" >>/dev/null

		cat <<EOF
## ${repo} Changes
$(changes)

EOF
		popd >>/dev/null
		rm -rf "${tmp_dir}/${repo}"
	done

	cat <<EOF

## Compatibility with CRI-O
Kata Containers ${runtime_version} is compatible with CRI-O ${crio_version}

## Compatibility with containerd
Kata Containers ${runtime_version} is compatible with contaienrd ${containerd_version}

## OCI Runtime Specification
Kata Containers ${runtime_version} support the OCI Runtime Specification [${oci_spec_version}][ocispec]

## Compatibility with Kubernetes
Kata Containers ${runtime_version} is compatible with Kubernetes ${kubernetes_version}

## Libseccomp Notices
The \`kata-agent\` binaries inside the Kata Containers images provided with this release are
statically linked with the following [GNU LGPL-2.1][lgpl-2.1] licensed libseccomp library.

* [\`libseccomp\`][libseccomp]

The \`kata-agent\` uses the libseccomp v${libseccomp_version} which is not modified from the upstream version.
However, in order to comply with the LGPL-2.1 (§6(a)), we attach the complete source code for the library.

If you want to use the \`kata-agent\` which is not statically linked with the library, you can build
a custom \`kata-agent\` that does not use the library from sources.
For the details, please check the [developer guide][custom-agent-doc].

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
[libseccomp]: ${libseccomp_url}
[lgpl-2.1]: https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html
[custom-agent-doc]: https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md#build-a-custom-kata-agent---optional
[limitations]: https://github.com/kata-containers/kata-containers/blob/${new_release}/docs/Limitations.md
[installation]: https://github.com/kata-containers/kata-containers/blob/${new_release}/docs/install
EOF
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
