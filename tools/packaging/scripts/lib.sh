#!/usr/bin/env bash
#
# Copyright (c) 2018-2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

export GOPATH=${GOPATH:-${HOME}/go}
export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"
export CC_BUILDER_REGISTRY="quay.io/kata-containers/cc-builders"
export PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-"no"}"

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

short_commit_length=10

hub_bin="hub-bin"

# Jenkins URL
jenkins_url="http://jenkins.katacontainers.io"
# Path where cached artifacts are found.
cached_artifacts_path="lastSuccessfulBuild/artifact/artifacts"

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

get_from_kata_deps() {
	local dependency="$1"
	versions_file="${this_script_dir}/../../../versions.yaml"

	command -v yq &>/dev/null || die 'yq command is not in your $PATH'
	result=$("yq" read -X "$versions_file" "$dependency")
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

get_config_and_patches() {
	if [ -z "${patches_path}" ]; then
		patches_path="${default_patches_dir}"
	fi
}

get_config_version() {
	get_config_and_patches
	config_version_file="${default_patches_dir}/../kata_config_version"
	if [ -f "${config_version_file}" ]; then
		cat "${config_version_file}"
	else
		die "failed to find ${config_version_file}"
	fi
}

# $1 - Repo's root dir
# $2 - The file we're looking for the last modification
get_last_modification() {
	local repo_root_dir="${1}"
	local file="${2}"

	# This is a workaround needed for when running this code on Jenkins
	git config --global --add safe.directory ${repo_root_dir} &> /dev/null

	dirty=""
	[ $(git status --porcelain | grep "${file#${repo_root_dir}/}" | wc -l) -gt 0 ] && dirty="-dirty"

	echo "$(git log -1 --pretty=format:"%H" ${file})${dirty}"
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
	local files="${this_script_dir}/../qemu \
		${this_script_dir}/../static-build/qemu.blacklist \
		${this_script_dir}/../static-build/scripts"

	sha256sum_from_files "$files"
}
