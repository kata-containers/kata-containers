#!/usr/bin/env bash
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

kata_build_dir=${1:-build}
kata_versions_yaml_file=${2:-""}

tar_path="${PWD}/kata-static.tar.xz"
kata_versions_yaml_file_path="${PWD}/${kata_versions_yaml_file}"

pushd "${kata_build_dir}"
tarball_content_dir="${PWD}/kata-tarball-content"
rm -rf "${tarball_content_dir}"
mkdir "${tarball_content_dir}"

for c in kata-static-*.tar.xz
do
	echo "untarring tarball "${c}" into ${tarball_content_dir}"
	tar -xvf "${c}" -C "${tarball_content_dir}"
done

pushd ${tarball_content_dir}
	shim="containerd-shim-kata-v2"
	shim_path=$(find . -name ${shim} | sort | head -1)
	prefix=${shim_path%"bin/${shim}"}

	echo "$(git describe --tags)" > ${prefix}/VERSION
	[[ -n "${kata_versions_yaml_file}" ]] && cp ${kata_versions_yaml_file_path} ${prefix}/
popd

echo "create ${tar_path}"
(cd "${tarball_content_dir}"; tar cvfJ "${tar_path}" --owner=0 --group=0 .)
popd
