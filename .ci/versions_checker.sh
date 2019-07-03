#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# The purpose of this script is to
# run the tag_repos.sh script that is in the
# packaging repository which checks the VERSION
# file from the components in order to verify
# that the VERSION matches between them.
# This ensures that the rest
# of the components are merged before the runtime

set -e

GOPATH=${GOPATH:-${HOME}/go}

PACKAGING_REPO="github.com/kata-containers/packaging"
RUNTIME_REPO="github.com/kata-containers/runtime"

go get -d "${PACKAGING_REPO}" || true

check_changes=$(git diff "${GOPATH}/src/${RUNTIME_REPO}/VERSION")
version_to_check=$(cat "${GOPATH}/src/${RUNTIME_REPO}/VERSION")

if [ ! -z "$check_changes" ]; then
	pushd "${GOPATH}/src/${PACKAGING_REPO}"
	./release/tag_repos.sh pre-release "${version_to_check}" 
	popd
fi
