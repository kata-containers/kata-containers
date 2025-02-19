#!/usr/bin/env bash
#
# Copyright (c) 2018-2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

export GOPATH=${GOPATH:-${HOME}/go}
export BUILDER_REGISTRY="${BUILDER_REGISTRY:-quay.io/kata-containers/builders}"
export PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-"no"}"

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export repo_root_dir="$(cd "${this_script_dir}/../../../" && pwd)"

short_commit_length=10

gh_cli="gh-cli"

#for cross build
CROSS_BUILD=${CROSS_BUILD-:}
BUILDX=""
PLATFORM=""
TARGET_ARCH=${TARGET_ARCH:-$(uname -m)}
ARCH=${ARCH:-$(uname -m)}
[ "${TARGET_ARCH}" == "aarch64" ] && TARGET_ARCH=arm64
TARGET_OS=${TARGET_OS:-linux}
[ "${CROSS_BUILD}" == "true" ] && BUILDX=buildx && PLATFORM="--platform=${TARGET_OS}/${TARGET_ARCH}"

install_yq() {
	pushd "${repo_root_dir}"
	./ci/install_yq.sh
	popd
}

get_from_kata_deps() {
  local dependency="$1 | explode(.)"
	versions_file="${this_script_dir}/../../../versions.yaml"

	command -v yq &>/dev/null || die 'yq command is not in your $PATH'
	result=$("yq" "$dependency" "$versions_file")
	[ "$result" = "null" ] && result=""
	echo "$result"
}

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

info() {
	echo >&2 "INFO: $*"
}

get_repo_hash() {
	local repo_dir=${1:-}
	[ -d "${repo_dir}" ] || die "${repo_dir} is not a directory"
	pushd "${repo_dir}" >>/dev/null
	git rev-parse --verify HEAD
	popd >>/dev/null
}

arch_to_golang()
{
	local -r arch="$1"

	case "$arch" in
		aarch64) echo "arm64";;
		ppc64le) echo "$arch";;
		riscv64) echo "$arch";;
		x86_64) echo "amd64";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: $arch";;
	esac
}

get_gh() {
	info "Get gh"

	if cmd=$(command -v gh); then
		gh_cli="${cmd}"
		return
	else
		gh_cli="${tmp_dir:-/tmp}/gh-cli"
	fi

	local goarch=$(arch_to_golang $(uname -m))
	curl -sSL https://github.com/cli/cli/releases/download/v2.37.0/gh_2.37.0_linux_${goarch}.tar.gz | tar -xz
	mv gh_2.37.0_linux_${goarch}/bin/gh "${gh_cli}"
	rm -rf gh_2.37.0_linux_amd64
}

get_kata_hash() {
	repo=$1
	ref=$2
	git ls-remote --heads --tags "https://github.com/${project}/${repo}.git" | grep "${ref}" | awk '{print $1}'
}

merge_two_hashes() {
	local hash1="${1}"
	local hash2="${2}"

	echo "${hash1}${hash2}" | sha256sum | cut -c1-9
}

# $1 - The file we're looking for the last modification
get_last_modification() {
	local file="${1}"

	pushd ${repo_root_dir} &> /dev/null

	dirty=""
	[ $(git status --porcelain | grep "${file#${repo_root_dir}/}" | wc -l) -gt 0 ] && dirty="-dirty"

	echo "$(git log -1 --abbrev=9 --pretty=format:"%h" ${file})${dirty}"
	popd &> /dev/null
}

# $1 - The tag to be pushed to the registry
# $2 - "yes" to use sudo, "no" otherwise
push_to_registry() {
	local tag="${1}"
	local use_sudo="${2:-"yes"}"

	if [ "${PUSH_TO_REGISTRY}" == "yes" ]; then
		if [ "${use_sudo}" == "yes" ]; then
			sudo docker push ${tag}
		else
			docker push ${tag}
		fi
	fi
}

get_kernel_image_name() {
	kernel_script_dir="${repo_root_dir}/tools/packaging/static-build/kernel"
	echo "${BUILDER_REGISTRY}:kernel-$(get_last_modification ${kernel_script_dir})-$(uname -m)"
}

sha256sum_from_files() {
	local files_in=${@:-}
	local files=""
	local shasum=""

	# Process the input files:
	#  - discard the files/directories that don't exist.
	#  - find the files if it is a directory
	for f in $files_in; do
		if [ -d "$f" ]; then
			files+=" $(find $f -type f)"
		elif [ -f "$f" ]; then
			files+=" $f"
		fi
	done
	# Return in case there is none input files.
	[ -n "$files" ] || return 0

	# Alphabetically sorting the files.
	files="$(echo $files | tr ' ' '\n' | LC_ALL=C sort -u)"
	# Concate the files and calculate a hash.
	shasum="$(cat $files | sha256sum -b)" || true
	if [ -n "$shasum" ];then
		# Return only the SHA field.
		echo $(awk '{ print $1 }' <<< $shasum)
	fi
}

calc_qemu_files_sha256sum() {
	local files="${repo_root_dir}/tools/packaging/qemu \
		${repo_root_dir}/tools/packaging/static-build/qemu.blacklist \
		${repo_root_dir}/tools/packaging/static-build/scripts"

	sha256sum_from_files "$files"
}

get_qemu_image_name() {
	qemu_script_dir="${repo_root_dir}/tools/packaging/static-build/qemu"
	echo "${BUILDER_REGISTRY}:qemu-$(get_last_modification ${qemu_script_dir})-$(uname -m)"
}

get_shim_v2_image_name() {
	shim_v2_script_dir="${repo_root_dir}/tools/packaging/static-build/shim-v2"
	echo "${BUILDER_REGISTRY}:shim-v2-go-$(get_from_kata_deps ".languages.golang.meta.newest-version")-rust-$(get_from_kata_deps ".languages.rust.meta.newest-version")-$(get_last_modification ${shim_v2_script_dir})-$(uname -m)"
}

get_ovmf_image_name() {
	ovmf_script_dir="${repo_root_dir}/tools/packaging/static-build/ovmf"
	echo "${BUILDER_REGISTRY}:ovmf-$(get_last_modification ${ovmf_script_dir})-$(uname -m)"
}

get_busybox_image_name() {
	busybox_script_dir="${repo_root_dir}/tools/packaging/static-build/busybox"
	echo "${BUILDER_REGISTRY}:busybox-$(get_last_modification "${busybox_script_dir}")-$(uname -m)"
}

get_virtiofsd_image_name() {
	ARCH=${ARCH:-$(uname -m)}
	case ${ARCH} in
	        "aarch64")
	                libc="musl"
	                ;;
	        "ppc64le")
	                libc="gnu"
	                ;;
	        "riscv64")
	                libc="gnu"
	                ;;
	        "s390x")
	                libc="gnu"
	                ;;
	        "x86_64")
	                libc="musl"
	                ;;
	esac

	virtiofsd_script_dir="${repo_root_dir}/tools/packaging/static-build/virtiofsd"
	echo "${BUILDER_REGISTRY}:virtiofsd-$(get_from_kata_deps ".externals.virtiofsd.toolchain")-${libc}-$(get_last_modification ${virtiofsd_script_dir})-$(uname -m)"
}

get_tools_image_name() {
	tools_script_dir="${repo_root_dir}/tools/packaging/static-build/tools"
	tools_dir="${repo_root_dir}/src/tools"
	libs_dir="${repo_root_dir}/src/libs"
	agent_dir="${repo_root_dir}/src/agent"

	echo "${BUILDER_REGISTRY}:tools-$(get_last_modification ${tools_dir})-$(get_last_modification ${libs_dir})-$(get_last_modification ${agent_dir})-$(get_last_modification ${tools_script_dir})-$(uname -m)"
}

get_agent_image_name() {
	libseccomp_hash=$(merge_two_hashes \
		"$(get_last_modification "${repo_root_dir}/ci/install_libseccomp.sh")" \
		"$(get_last_modification "${repo_root_dir}/tools/packaging/kata-deploy/local-build/kata-deploy-copy-libseccomp-installer.sh")")
	agent_dir="${repo_root_dir}/tools/packaging/static-build/agent"
	rust_toolchain="$(get_from_kata_deps ".languages.rust.meta.newest-version")"

	echo "${BUILDER_REGISTRY}:agent-${libseccomp_hash}-$(get_last_modification ${agent_dir})-${rust_toolchain}-$(uname -m)"
}

get_coco_guest_components_image_name() {
	coco_guest_components_script_dir="${repo_root_dir}/tools/packaging/static-build/coco-guest-components"
	echo "${BUILDER_REGISTRY}:coco-guest-components-$(get_from_kata_deps ".externals.coco-guest-components.toolchain")-$(get_last_modification ${coco_guest_components_script_dir})-$(uname -m)"
}

get_pause_image_name() {
	pause_image_script_dir="${repo_root_dir}/tools/packaging/static-build/pause-image"
	echo "${BUILDER_REGISTRY}:pause-image-$(get_last_modification ${pause_image_script_dir})-$(uname -m)"
}
