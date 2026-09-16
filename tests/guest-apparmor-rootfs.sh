#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rootfs_script="${repo_root}/tools/osbuilder/rootfs-builder/rootfs.sh"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

# The Windows checkout may use CRLF for untouched upstream files. Keep this
# test independent of checkout line endings by sourcing normalized copies.
normalized_builder_dir="${tmp_dir}/rootfs-builder"
normalized_scripts_dir="${tmp_dir}/scripts"
mkdir -p "${normalized_builder_dir}" "${normalized_scripts_dir}"
perl -pe 's/\r\n/\n/g; s/\r/\n/g' "${rootfs_script}" \
	> "${normalized_builder_dir}/rootfs.sh"
perl -pe 's/\r\n/\n/g; s/\r/\n/g' \
	"${repo_root}/tools/osbuilder/scripts/lib.sh" \
	> "${normalized_scripts_dir}/lib.sh"

run_rootfs_function() {
	ROOTFS_BUILDER_NO_MAIN=yes \
	ROOTFS_BUILDER_SCRIPT_DIR="${normalized_builder_dir}" \
	ROOTFS_DIR="$1" \
	GUEST_APPARMOR="$2" \
	AGENT_INIT="$3" \
	GUEST_APPARMOR_PROFILE_TARBALL="${4:-}" \
	bash -c 'source "$1"; install_guest_apparmor_assets' _ \
		"${normalized_builder_dir}/rootfs.sh"
}

plain_rootfs="${tmp_dir}/plain"
run_rootfs_function "${plain_rootfs}" no yes
[[ ! -e "${plain_rootfs}/etc/apparmor.d" ]]

agent_init_rootfs="${tmp_dir}/agent-init"
run_rootfs_function "${agent_init_rootfs}" yes yes
[[ -d "${agent_init_rootfs}/etc/apparmor.d" ]]

systemd_rootfs="${tmp_dir}/systemd"
mkdir -p "${systemd_rootfs}/usr/lib/systemd/system"
touch "${systemd_rootfs}/usr/lib/systemd/system/apparmor.service"
run_rootfs_function "${systemd_rootfs}" yes no
[[ -L "${systemd_rootfs}/etc/systemd/system/basic.target.wants/apparmor.service" ]]
[[ "$(readlink "${systemd_rootfs}/etc/systemd/system/basic.target.wants/apparmor.service")" == "/usr/lib/systemd/system/apparmor.service" ]]

if command -v zstd >/dev/null 2>&1; then
	profile_source="${tmp_dir}/profiles"
	mkdir -p "${profile_source}"
	printf 'profile kata-default {\n}\n' > "${profile_source}/kata-default"
	safe_archive="${tmp_dir}/profiles.tar.zst"
	tar --zstd -cf "${safe_archive}" -C "${profile_source}" kata-default
	archive_rootfs="${tmp_dir}/archive"
	run_rootfs_function "${archive_rootfs}" yes yes "${safe_archive}"
	[[ -f "${archive_rootfs}/etc/apparmor.d/kata-default" ]]

	unsafe_archive="${tmp_dir}/unsafe-profiles.tar.zst"
	tar --zstd --transform='s#^kata-default$#../escape#' \
		-cf "${unsafe_archive}" -C "${profile_source}" kata-default
	if run_rootfs_function "${tmp_dir}/unsafe" yes yes "${unsafe_archive}"; then
		echo "unsafe profile archive was accepted" >&2
		exit 1
	fi
else
	echo "Profile archive checks skipped: zstd is unavailable"
fi

echo "Guest AppArmor rootfs checks passed"
