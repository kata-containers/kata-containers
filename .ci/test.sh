#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

CI=${CI:-}
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
readonly toplevel_mk="${script_dir}/../Makefile"
source "${script_dir}/lib.sh"

make_target() {
	target=$1
	dir=$2

	pushd "${script_dir}/.." >> /dev/null
	if [ -n "${CI}" ] &&  ! git whatchanged  origin/master..HEAD  "${dir}" | grep "${dir}" >> /dev/null; then
		echo "Not changes in ${dir}"
		return
	fi
	popd >> /dev/null
	echo "Changes found in $dir"
	make -f "${toplevel_mk}" ${target}
}

make_target test-release-tools "release/"
make_target test-packaging-tools "obs-packaging/"
make_target test-static-build "static-build/"
