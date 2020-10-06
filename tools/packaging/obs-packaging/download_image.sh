#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(cd $(dirname "${BASH_SOURCE[0]}") && pwd)
source "${script_dir}/scripts/obs-docker.sh"
source "${script_dir}/scripts/pkglib.sh"

handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

BRANCH=${1:-master}
agent_repository="https://github.com/kata-containers/agent.git"
kata_version_url="https://raw.githubusercontent.com/kata-containers/runtime/${BRANCH}/VERSION"

echo "Download kata image from branch ${BRANCH}"

if ! version=$(curl -s --fail -L "${kata_version_url}"); then
	die "failed to get version from branch ${BRANCH}"
fi

if ! out=$(git ls-remote "${agent_repository}"); then
	die "failed to query agent git repo"
fi

if ! tag_info=$(echo "$out" | grep "${version}^{}"); then
	die "failed to find version info $version: ${out}"
fi

commit=$(echo "$tag_info" | awk '{print $1}')
echo "$commit"

agent_repository="github.com/kata-containers/agent"
tarball_name="kata-containers-${version}-${commit:0:${short_commit_length}}-$(uname -m).tar.gz"
image_url="https://${agent_repository}/releases/download/${version}/${tarball_name}"
#curl -OL "${image_url}"
#tar xvf "${tarball_name}"
