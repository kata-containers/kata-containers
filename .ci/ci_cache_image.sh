#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

function handle_error {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

WORKSPACE=${WORKSPACE:-$(pwd)}
kata_dir="/usr/share/kata-containers/"

cache_build() {
        type="${1}"

        if [ "${type}" == "image" ]; then
                link="${kata_dir}/kata-containers.img"
        else
                link="${kata_dir}/kata-containers-${type}.img"
        fi
        path=$(readlink -f ${link})
        echo $(basename "${path}") >  "latest-${type}"
        sudo cp  "${path}" "${kata_dir}/osbuilder-${type}.yaml"  .

        sudo chown -R "${USER}:${USER}" ./

        sha256sum "$(cat latest-${type})" > "sha256sum-${type}"
        sha256sum -c "sha256sum-${type}"

        tar -cJf "$(cat latest-${type}).tar.xz" "$(cat latest-${type})"

        sha256sum "$(cat latest-${type}).tar.xz" > "sha256sum-${type}-tarball"
        sha256sum -c "sha256sum-${type}-tarball"
        rm "$(cat latest-${type})"
}

mkdir -p "${WORKSPACE}/artifacts"
pushd "${WORKSPACE}/artifacts"
cache_build image
cache_build initrd

ls -la "${WORKSPACE}/artifacts/"
popd
sync

