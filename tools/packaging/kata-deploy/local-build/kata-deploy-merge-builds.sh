#!/bin/bash
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

kata_build_dir=${1:-build}
tar_path="${PWD}/kata-static.tar.xz"

pushd "${kata_build_dir}"
tarball_content_dir="${PWD}/kata-tarball-content"
rm -rf "${tarball_content_dir}"
mkdir "${tarball_content_dir}"

for c in kata-static-*.tar.xz
do
    echo "untarring tarball "${c}" into ${tarball_content_dir}"
    tar -xvf "${c}" -C "${tarball_content_dir}"
done

echo "create ${tar_path}"
(cd "${tarball_content_dir}"; tar cvfJ "${tar_path}" .)
popd
