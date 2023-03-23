#!/usr/bin/env bash
#
# Copyright (c) 2018-2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

export GOPATH=${GOPATH:-${HOME}/go}
export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"
export BUILDER_REGISTRY="quay.io/kata-containers/builders"
export PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-"no"}"

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export repo_root_dir="$(cd "${this_script_dir}/../../../" && pwd)"

short_commit_length=10

hub_bin="hub-bin"

kata_versions_file="${script_dir}/../../../versions.yaml"

clone_tests_repo() {
	# KATA_CI_NO_NETWORK is (has to be) ignored if there is
	# no existing clone.
	if [ -d "${tests_repo_dir}" ] && [ -n "${KATA_CI_NO_NETWORK:-}" ]; then
		return
	fi

	go get -d -u "$tests_repo" || true
}

install_yq() {
	clone_tests_repo
	pushd "$tests_repo_dir"
	.ci/install_yq.sh
	popd
}


get_package_version_from_kata_yaml()
{
    local yq_path="$1"
    local yq_version
    local yq_args

	typeset -r yq=$(command -v yq || command -v "${GOPATH}/bin/yq" || echo "${GOPATH}/bin/yq")
	if [ ! -f "$yq" ]; then
		source "$yq_file"
	fi

    yq_version=$($yq -V)
    case $yq_version in
    *"version "[1-3]*)
        yq_args="r -X - ${yq_path}"
        ;;
    *)
        yq_args="e .${yq_path} -"
        ;;
    esac

	PKG_VERSION="$(cat "${kata_versions_file}" | $yq ${yq_args})"

	[ "$?" == "0" ] && [ "$PKG_VERSION" != "null" ] && echo "$PKG_VERSION" || echo ""
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

build_hub() {
	info "Get hub"

	if cmd=$(command -v hub); then
		hub_bin="${cmd}"
		return
	else
		hub_bin="${tmp_dir:-/tmp}/hub-bin"
	fi

	local hub_repo="github.com/github/hub"
	local hub_repo_dir="${GOPATH}/src/${hub_repo}"
	[ -d "${hub_repo_dir}" ] || git clone --quiet --depth 1 "https://${hub_repo}.git" "${hub_repo_dir}"
	pushd "${hub_repo_dir}" >>/dev/null
	git checkout master
	git pull
	./script/build -o "${hub_bin}"
	popd >>/dev/null
}

arch_to_golang()
{
	local -r arch="$1"

	case "$arch" in
		aarch64) echo "arm64";;
		ppc64le) echo "$arch";;
		x86_64) echo "amd64";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: $arch";;
	esac
}

get_kata_hash() {
	repo=$1
	ref=$2
	git ls-remote --heads --tags "https://github.com/${project}/${repo}.git" | grep "${ref}" | awk '{print $1}'
}

# $1 - The file we're looking for the last modification
get_last_modification() {
	local file="${1}"

	pushd ${repo_root_dir} &> /dev/null
	# This is a workaround needed for when running this code on Jenkins
	git config --global --add safe.directory ${repo_root_dir} &> /dev/null

	dirty=""
	[ $(git status --porcelain | grep "${file#${repo_root_dir}/}" | wc -l) -gt 0 ] && dirty="-dirty"

	echo "$(git log -1 --pretty=format:"%H" ${file})${dirty}"
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
	echo "${BUILDER_REGISTRY}:shim-v2-go-$(get_package_version_from_kata_yaml "languages.golang.meta.newest-version")-rust-$(get_package_version_from_kata_yamls "languages.rust.meta.newest-version")-$(get_last_modification ${shim_v2_script_dir})-$(uname -m)"
}

get_virtiofsd_image_name() {
	ARCH=$(uname -m)
	case ${ARCH} in
	        "aarch64")
	                libc="musl"
	                ;;
	        "ppc64le")
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
	echo "${BUILDER_REGISTRY}:virtiofsd-$(get_package_version_from_kata_yaml "externals.virtiofsd.toolchain")-${libc}-$(get_last_modification ${virtiofsd_script_dir})-$(uname -m)"
}
