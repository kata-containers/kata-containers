#
# Copyright (c) 2018 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0

# List of distros not to test, when running all tests with test_images.sh
typeset -a skipWhenTestingAll
typeset -a distros
arch="$(uname -m)"
sdir="${BASH_SOURCE[0]%/*}"
for distro in $(${sdir}/../rootfs-builder/rootfs.sh -l); do
	distros+=("${distro}")
done
test_distros=()
test_distros+=("ubuntu")

skipForRustDistros=()
skipForRustDistros+=("alpine")

skipForRustArch=()
skipForRustArch+=("ppc64le")
skipForRustArch+=("s390x")

distro_in_set() {
	local d=$1
	shift
	local dt
	for dt in "$@"; do
		if [ "${dt}" == "${d}" ]; then
			return 0
		fi
	done
	return 1
}
