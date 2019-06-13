#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

readonly script_dir=$(dirname $(readlink -f "$0"))

# Source to trap error line number
# shellcheck source=../lib/common.bash
source "${script_dir}/../lib/common.bash"

WORKSPACE=${WORKSPACE:-$(pwd)}
kata_dir="/usr/share/kata-containers"

cache_built_kernel() {
	kernel_path="${1}"
	real_path=$(readlink -f "${kernel_path}")
	#Get version from binary (format: vmlinu{z|x}-4.19.24-25)
	kernel_binary=$(basename "${real_path}")
	# Latest if the file to say what is the cached version
	echo "${kernel_binary}" | cut -d- -f2- >  "latest"
	cp "${real_path}" ./

        sudo chown -R "${USER}:${USER}" ./
	sha256sum "${kernel_binary}" >> "sha256sum-kernel"
	cat sha256sum-kernel
}

mkdir -p "${WORKSPACE}/artifacts"
pushd "${WORKSPACE}/artifacts"
rm -f sha256sum-kernel
for k in "${kata_dir}/"*.container; do
	echo "Adding ${k}"
	cache_built_kernel "${k}"
done
echo "artifacts:"
ls -la "${WORKSPACE}/artifacts/"
popd
#The script is running in a VM as part of a CI Job, the artifacts will be
#collected by the CI master node, sync to make sure any data is updated.
sync
