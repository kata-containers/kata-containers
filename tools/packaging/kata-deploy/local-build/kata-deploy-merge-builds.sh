#!/usr/bin/env bash
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[[ -z "${DEBUG}" ]] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "${this_script_dir}/../../../../" && pwd)"

kata_build_dir=${1:-build}
kata_versions_yaml_file=${2:-""}
output_tarball_name=${3:-kata-static.tar.zst}

tar_path="${PWD}/${output_tarball_name}"
kata_versions_yaml_file_path="${PWD}/${kata_versions_yaml_file}"

pushd "${kata_build_dir}"
tarball_content_dir="${PWD}/kata-tarball-content"
rm -rf "${tarball_content_dir}"
mkdir "${tarball_content_dir}"

for c in kata-static-*.tar.zst
do
	echo "untarring tarball \"${c}\" into ${tarball_content_dir}"
	tar --zstd -xvf "${c}" -C "${tarball_content_dir}"
done

pushd "${tarball_content_dir}"
	any_binary=$(find . -path "*/opt/kata/bin/*" -type f | head -1)
	if [[ -z "${any_binary}" ]]; then
		echo "Error: No binaries found in opt/kata/bin/" >&2
		exit 1
	fi
	prefix=${any_binary%bin/*}

	if [[ "${RELEASE:-no}" == "yes" ]] && [[ -f "${repo_root_dir}/VERSION" ]]; then
		# In this case the tag was not published yet,
		# thus we need to rely on the VERSION file.
		cp "${repo_root_dir}/VERSION" "${prefix}/"
	else
		git describe --tags > "${prefix}/VERSION"
	fi
	[[ -n "${kata_versions_yaml_file}" ]] && cp "${kata_versions_yaml_file_path}" "${prefix}/"
popd

echo "create ${tar_path}"
(cd "${tarball_content_dir}"; tar --zstd -cvf "${tar_path}" --owner=0 --group=0 .)
popd
