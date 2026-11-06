#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rootfs_script="${repo_root}/tools/osbuilder/rootfs-builder/rootfs.sh"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

# Run one function from rootfs.sh against a prepared rootfs tree.
#
# The real script executes main() on load, so it cannot simply be sourced.
# Extract only the function definition and drive it in a subshell instead of
# adding an entry-point hook to the production script.
run_rootfs_function() {
	local function_name="$1"
	local target_dir="$2"

	{
		printf 'info() { printf "INFO: %%s\\n" "$*"; }\n'
		awk "/^${function_name}\\(\\)/,/^}/" "${rootfs_script}"
		printf '%s\n' "${function_name}"
	} > "${tmp_dir}/driver.sh"

	ROOTFS_DIR="${target_dir}" bash "${tmp_dir}/driver.sh"
}

check_plain_rootfs_is_untouched() {
	local rootfs="${tmp_dir}/plain"
	mkdir -p "${rootfs}"

	run_rootfs_function disable_distro_apparmor_service "${rootfs}"

	# No apparmor userspace in this rootfs, so nothing may be added.
	[[ ! -e "${rootfs}/usr/lib/systemd/system-preset/10-kata-apparmor.preset" ]]
	[[ ! -e "${rootfs}/etc/systemd/system/apparmor.service" ]]
}

check_service_is_disabled_not_masked() {
	local rootfs="${tmp_dir}/systemd"
	mkdir -p "${rootfs}/lib/systemd/system"
	touch "${rootfs}/lib/systemd/system/apparmor.service"

	run_rootfs_function disable_distro_apparmor_service "${rootfs}"

	local preset="${rootfs}/usr/lib/systemd/system-preset/10-kata-apparmor.preset"
	[[ -f "${preset}" ]]
	grep -q '^disable apparmor\.service$' "${preset}"

	# A preset expresses "do not enable by default". Masking would be a
	# stronger claim than the contract makes, so it must not happen.
	[[ ! -e "${rootfs}/etc/systemd/system/apparmor.service" ]]
}

check_existing_enablement_is_removed() {
	local rootfs="${tmp_dir}/enabled"
	mkdir -p "${rootfs}/lib/systemd/system"
	mkdir -p "${rootfs}/etc/systemd/system/multi-user.target.wants"
	touch "${rootfs}/lib/systemd/system/apparmor.service"
	ln -sf "/lib/systemd/system/apparmor.service" \
		"${rootfs}/etc/systemd/system/multi-user.target.wants/apparmor.service"

	run_rootfs_function disable_distro_apparmor_service "${rootfs}"

	# The preset and the on-disk state have to agree.
	[[ ! -e "${rootfs}/etc/systemd/system/multi-user.target.wants/apparmor.service" ]]
	[[ -f "${rootfs}/usr/lib/systemd/system-preset/10-kata-apparmor.preset" ]]
}

check_usr_lib_location_is_recognised() {
	local rootfs="${tmp_dir}/usr-lib"
	mkdir -p "${rootfs}/usr/lib/systemd/system"
	touch "${rootfs}/usr/lib/systemd/system/apparmor.service"

	run_rootfs_function disable_distro_apparmor_service "${rootfs}"

	[[ -f "${rootfs}/usr/lib/systemd/system-preset/10-kata-apparmor.preset" ]]
}

check_idempotent() {
	local rootfs="${tmp_dir}/twice"
	mkdir -p "${rootfs}/lib/systemd/system"
	touch "${rootfs}/lib/systemd/system/apparmor.service"

	run_rootfs_function disable_distro_apparmor_service "${rootfs}"
	run_rootfs_function disable_distro_apparmor_service "${rootfs}"

	local preset="${rootfs}/usr/lib/systemd/system-preset/10-kata-apparmor.preset"
	[[ "$(grep -c '^disable apparmor\.service$' "${preset}")" -eq 1 ]]
}

check_no_apparmor_options_remain() {
	# The opt-in build switches are gone; the parser is installed
	# unconditionally instead. Guard against them creeping back.
	local leftovers
	leftovers="$(grep -nE 'GUEST_APPARMOR|ROOTFS_BUILDER_(SCRIPT_DIR|NO_MAIN)' "${rootfs_script}" || true)"
	if [[ -n "${leftovers}" ]]; then
		echo "unexpected guest AppArmor build options in rootfs.sh:" >&2
		echo "${leftovers}" >&2
		return 1
	fi
}

check_plain_rootfs_is_untouched
check_service_is_disabled_not_masked
check_existing_enablement_is_removed
check_usr_lib_location_is_recognised
check_idempotent
check_no_apparmor_options_remain

echo "Guest AppArmor rootfs checks passed"
