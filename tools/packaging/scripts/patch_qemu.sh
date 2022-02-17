#!/usr/bin/env bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname $(readlink -f "$0"))
handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "error:"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

usage(){
	echo "$0 <qemu_version> <patches_dir>"
}

qemu_version="${1:-}"
[ "${qemu_version}" == "" ] && usage && exit 1

patches_dir="${2:-}"
[ "${patches_dir}" == "" ] && usage && exit 1

apply_patches="${script_dir}/apply_patches.sh"

stable_branch=$(cat VERSION | awk 'BEGIN{FS=OFS="."}{print $1 "." $2 ".x"}')
patch_version=$(cat VERSION | awk 'BEGIN{FS=OFS="."}{print $3}')

if (( $patch_version >= 50));then
	echo "Found qemu dev version: Qemu uses patch version +50 to identify new development tree."
	echo "Patches for base version ${stable_branch} are not used for $(cat VERSION)"
else
	echo "Apply patches for base version ${stable_branch}"
	"${apply_patches}" "${patches_dir}/${stable_branch}"
fi

echo "Apply patches for specific qemu version ${qemu_version}"
"${apply_patches}" "${patches_dir}/tag_patches/${qemu_version}"
