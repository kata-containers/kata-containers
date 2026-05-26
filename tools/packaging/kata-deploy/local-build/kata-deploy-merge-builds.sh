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
known_tarballs=${4:-""}
merge_mode=${5:-merge}

tar_path=$(readlink -f "${output_tarball_name}")
if [[ -n "${kata_versions_yaml_file}" ]]; then
	# We pushd into ${kata_build_dir} below, so resolve this path before that.
	case "${kata_versions_yaml_file}" in
		/*) ;;
		*) kata_versions_yaml_file="${PWD}/${kata_versions_yaml_file}" ;;
	esac
	kata_versions_yaml_file=$(readlink -f "${kata_versions_yaml_file}")
fi

pushd "${kata_build_dir}"
tarball_content_dir="${PWD}/kata-tarball-content"
rm -rf "${tarball_content_dir}"
mkdir "${tarball_content_dir}"

for c in ${known_tarballs:-kata-static-*.tar.zst}; do
	if [[ ! -f "${c}" ]]; then
		# When the caller provided an explicit allowlist, a missing entry
		# means the build produced an incomplete artifact set; fail loudly
		# instead of silently shipping a partial final tarball.
		if [[ -n "${known_tarballs}" ]]; then
			echo "ERROR: required tarball \"${c}\" is missing in ${PWD}" >&2
			exit 1
		fi
		echo "skipping missing tarball \"${c}\""
		continue
	fi
	if [[ "${merge_mode}" == "passthrough" ]]; then
		echo "copying tarball \"${c}\" into ${tarball_content_dir}"
		cp "${c}" "${tarball_content_dir}/"
	else
		echo "untarring tarball \"${c}\" into ${tarball_content_dir}"
		tar --zstd -xvf "${c}" -C "${tarball_content_dir}"
	fi
done

pushd "${tarball_content_dir}"
	if [[ "${merge_mode}" == "passthrough" ]]; then
		[[ -n "${kata_versions_yaml_file}" ]] && cp "${kata_versions_yaml_file}" .
	else
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
		[[ -n "${kata_versions_yaml_file}" ]] && cp "${kata_versions_yaml_file}" "${prefix}/"
	fi
popd

echo "create ${tar_path}"
(cd "${tarball_content_dir}"; tar --zstd -cvf "${tar_path}" --owner=0 --group=0 .)
popd
