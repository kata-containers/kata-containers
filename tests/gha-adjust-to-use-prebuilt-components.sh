#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "${this_script_dir}/../" && pwd)"

base_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build"
build_dir="${base_dir}/build"

function main() {
    artifacts_dir="${1:-}"
    asset="${2:-}"

    if [ -z "${artifacts_dir}" ]; then
        echo "The artefacts directory must be passed as the first argument to this script."
        exit 1
    fi

    if [ -z "${asset}" ]; then
        echo "The asset must be passed as the second argument to this script."
        exit 1
    fi

    mv ${artifacts_dir} ${build_dir}
    sed -i "s/\(^${asset}-tarball:\).*/\1/g" ${base_dir}/Makefile
}

main "$@"
