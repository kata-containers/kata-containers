#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

CI=${CI:-}
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly toplevel_mk="${script_dir}/../Makefile"
source "${script_dir}/../scripts/lib.sh"

make_target() {
	target=$1
	dir=$2

	pushd "${script_dir}/.." >>/dev/null

	if ! git diff --name-only origin/master..HEAD ${dir} | grep ${dir}; then
		echo "Not changes in ${dir}"
		return
	fi
	case "${target}" in
	test-packaging-tools)
		skip_msg="skip $target see https://github.com/kata-containers/packaging/issues/72"
		[ -n "${CI}" ] && echo "${skip_msg}" && return
		;;

	esac

	popd >>/dev/null
	echo "Changes found in ${dir}"
	make -f "${toplevel_mk}" "${target}"
}

# Check that kata_confing_version file is updated
# when there is any change in the kernel directory.
# If there is a change in the directory, but the config
# version is not updated, return error.
check_kata_kernel_version() {
	kernel_version_file="kernel/kata_config_version"
	modified_files=$(git diff --name-only origin/master..HEAD)
	echo "Check Changes in kernel"
	git diff origin/master..HEAD ${kernel_version_file}
	git diff --name-only origin/master..HEAD
	if git whatchanged origin/master..HEAD "kernel/" | grep "kernel/" >>/dev/null; then
		echo "Kernel directory has changes check $kernel_version_file changed"
		echo "$modified_files" | grep "$kernel_version_file" ||
			die "Please bump version in $kernel_version_file"
	fi
	echo "OK - config version file was updated"

}

make_target test-release-tools "release/"
make_target test-packaging-tools "obs-packaging/"
make_target test-static-build "static-build/"
make_target cmd-kata-pkgsync "cmd/kata-pkgsync"

[ -n "${CI}" ] && check_kata_kernel_version
