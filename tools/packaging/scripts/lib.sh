#!/usr/bin/env bash
#
# Copyright (c) 2018-2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

export GOPATH=${GOPATH:-${HOME}/go}
export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

short_commit_length=10

hub_bin="hub-bin"

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
	GOPATH=${GOPATH:-${HOME}/go}
	versions_file="${this_script_dir}/../../../versions.yaml"

	#make sure yq is installed
	install_yq >&2

	result=$("${GOPATH}/bin/yq" read -X "$versions_file" "$dependency")
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
