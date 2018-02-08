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
