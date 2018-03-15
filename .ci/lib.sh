#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

_runtime_repo="github.com/kata-containers/runtime"
# FIXME: Issue https://github.com/kata-containers/packaging/issues/1
_versions_file="$GOPATH/src/github.com/clearcontainers/runtime/versions.txt"

export KATA_RUNTIME=${KATA_RUNTIME:-cc}

# If we fail for any reason a message will be displayed
die(){
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

function clone_and_build() {
	github_project="$1"
	make_target="$2"
	project_dir="${GOPATH}/src/${github_project}"

	echo "Retrieve repository ${github_project}"
	go get -d ${github_project} || true

	# fixme: once tool to parse and get branches from github is
	# completed, add it here to fetch branches under testing

	pushd ${project_dir}

	echo "Build ${github_project}"
	if [ ! -f Makefile ]; then
		echo "Run autogen.sh to generate Makefile"
		bash -f autogen.sh
	fi

	make

	popd
}

function clone_build_and_install() {
	clone_and_build $1 $2
	pushd "${GOPATH}/src/${1}"
	echo "Install repository ${1}"
	sudo -E PATH=$PATH KATA_RUNTIME=${KATA_RUNTIME} make install
	popd
}

function get_cc_versions(){
	# This is needed in order to retrieve the version for qemu-lite
	cc_runtime_repo="github.com/clearcontainers/runtime"
	go get -d -u -v "$cc_runtime_repo" || true
	[ ! -f "$_versions_file" ] && { echo >&2 "ERROR: cannot find $_versions_file"; exit 1; }
	source "$_versions_file"
}


function apply_depends_on() {
	pushd "${GOPATH}/src/${kata_repo}"
	label_lines=$(git log --format=%s%b master.. | grep "Depends-on:" || true)
	if [ "${label_lines}" == "" ]; then
		popd
		return 0
	fi

	nb_lines=$(echo "${label_lines}" | wc -l)

	repos_found=()
	for i in $(seq 1 "${nb_lines}")
	do
		label_line=$(echo "${label_lines}" | sed "${i}q;d")
		label_str=$(echo "${label_line}" | awk '{print $2}')
		repo=$(echo "${label_str}" | cut -d'#' -f1)
		if [[ "${repos_found[@]}" =~ "${repo}" ]]; then
			echo "Repository $repo was already defined in a 'Depends-on:' tag."
			echo "Only one repository per tag is allowed."
			return 1
		fi
		repos_found+=("$repo")
		pr_id=$(echo "${label_str}" | cut -d'#' -f2)

		echo "This PR depends on repository: ${repo} and pull request: ${pr_id}"
		if [ ! -d "${GOPATH}/src/${repo}" ]; then
			go get -d "$repo" || true
		fi

		pushd "${GOPATH}/src/${repo}"
		echo "Fetching pull request: ${pr_id} for repository: ${repo}"
		git fetch origin "pull/${pr_id}/head" && git checkout FETCH_HEAD && git rebase origin/master
		popd
	done

	popd
}
