#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This script will request package creation in OBS.
# create a repository under:
# https://build.opensuse.org/project/show/home:katacontainers
# Generate package files: rpm spec, deb source files.
# Send a request to OBS to build the packages in its servers

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

[ -z "${DEBUG:-}" ] || set -x
readonly script_dir=$(dirname $(readlink -f "$0"))
readonly packaging_dir="${script_dir}/../.."

# Kata branch where packages will be based
BRANCH=${BRANCH:-master}

# Name of OBS branch to to push
OBS_BRANCH="${OBS_BRANCH:-testing}"

if [ "${CI:-}" == "true" ] && [ "${GITHUB_PR:-}" != "" ]; then
	OBS_BRANCH="packaging-PR-${GITHUB_PR}"
fi

# Push to anywhere, variable used by release scripts to push
PUSH=1
BUILD_HEAD=${BUILD_HEAD:-${CI:-}}

if [ "${CI:-}" == "true" ]; then
	SUBPROJECT_TYPE="ci"
else
	SUBPROJECT_TYPE="releases"
fi
# Name of the OBS subproject under:
# https://build.opensuse.org/project/subprojects/home:katacontainers
OBS_SUBPROJECT="${SUBPROJECT_TYPE}:$(uname -m):${OBS_BRANCH}"

export BUILD_HEAD
export BRANCH
export OBS_BRANCH
export OBS_SUBPROJECT
export PUSH

# azure: Export in all pipeline tasks
echo "##vso[task.setvariable variable=OBS_SUBPROJECT;]${OBS_SUBPROJECT}"

echo "INFO: BUILD_HEAD=${BUILD_HEAD}"
echo "INFO: BRANCH=${BRANCH}"
echo "INFO: OBS_BRANCH=${OBS_SUBPROJECT}"
echo "INFO: PUSH=${PUSH}"
echo "INFO: SUBPROJECT_TYPE=${SUBPROJECT_TYPE}"

# Export in all pipeline tasks
cd "${packaging_dir}/obs-packaging" || exit 1
gen_versions_cmd="./gen_versions_txt.sh"
if [ "${BUILD_HEAD}" = "true" ]; then
	echo "Building for head gen versions ..."
	gen_versions_cmd+=" --head"
fi

${gen_versions_cmd} "${BRANCH}"

# print versions just for debug/info
cat versions.txt
export NEW_VERSION=$(curl -s -L https://raw.githubusercontent.com/kata-containers/runtime/${BRANCH}/VERSION)
create_repo_cmd="./create-repo-branch.sh"
if [ "${CI:-}" = "true" ]; then
	create_repo_cmd+=" --ci"
fi
create_repo_cmd+=" ${OBS_BRANCH}"
script -qefc bash -c "${create_repo_cmd}"
script -qefc bash -c './build_from_docker.sh ${NEW_VERSION}'
