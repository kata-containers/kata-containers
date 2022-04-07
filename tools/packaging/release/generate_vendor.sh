#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script_name="$(basename "${BASH_SOURCE[0]}")"

# This is very much error prone in case we re-structure our
# repos again, but it's also used in a few other places :-/
repo_dir="${script_dir}/../../.."

function usage() {

	cat <<EOF
Usage: ${script_name} tarball-name
This script creates a tarball with all the cargo vendored code
that a distro would need to do a full build of the project in
a disconnected environment, generating a "tarball-name" file.

EOF

}

create_vendor_tarball() {
	vendor_dir_list=""
	pushd ${repo_dir}
		for i in $(find . -name 'Cargo.lock'); do
			dir="$(dirname $i)"
			pushd "${dir}"
				[ -d .cargo ] || mkdir .cargo
				cargo vendor >> .cargo/config
				vendor_dir_list+=" $dir/vendor $dir/.cargo/config"
				echo "${vendor_dir_list}"
			popd
		done
	popd
	
	tar -cvzf ${1} ${vendor_dir_list}
}

main () {
	[ $# -ne 1 ] && usage && exit 0
	create_vendor_tarball ${1}
}

main "$@"
