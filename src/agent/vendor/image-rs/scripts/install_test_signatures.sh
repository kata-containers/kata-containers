#!/bin/bash
#
# Copyright (c) 2022 Alibaba Cloud
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

[ -n "${DEBUG:-}" ] && set -o xtrace

script_dir="$(dirname $(readlink -f $0))"
test_artifacts_dir="${script_dir}/../test_data/simple-signing-scheme"
rootfs_quay_verification_directory="/etc/containers/quay_verification"

if [ "${1:-}" = "install" ]; then
    sudo mkdir -p "${rootfs_quay_verification_directory}/signatures"
    sudo tar -zvxf "${test_artifacts_dir}/signatures.tar" -C "${rootfs_quay_verification_directory}/signatures"

elif [ "${1:-}" = "clean" ]; then
    sudo rm -rf "${rootfs_quay_verification_directory}/signatures"

else
    echo >&2 "ERROR: Wrong or missing argument: '${1:-}'"

fi

