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

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

GOPATH=${GOPATH:-${HOME}/go}

PACKAGING_REPO="github.com/kata-containers/packaging"
RUNTIME_REPO="github.com/kata-containers/runtime"
KATA_BRANCH=${target_branch:-master}

go get -d "${PACKAGING_REPO}" || true

if ! check_changes=$(git diff --name-only "origin/${KATA_BRANCH}" | grep VERSION); then
	echo "No changes in VERSION file - this is not a bump - nothing to check"
	exit 0
fi
version_to_check=$(cat "${GOPATH}/src/${RUNTIME_REPO}/VERSION")

if [ ! -z "$check_changes" ]; then
	echo "Changes detected on VERSION"
	echo "Check versions in branch ${KATA_BRANCH}"
	pushd "${GOPATH}/src/${PACKAGING_REPO}"
	./release/tag_repos.sh -b "${KATA_BRANCH}" pre-release "${version_to_check}"
	popd
fi
