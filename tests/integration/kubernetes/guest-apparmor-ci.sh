#!/usr/bin/env bash
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0

# CI ONLY: configure a disposable GitHub-hosted runner after kata-deploy.
# No kubectl calls: this script neither discovers nor changes an existing cluster.
# The real repository test policy is a temporary host CRI fixture, NOT a product
# dependency. Host/guest name equality is NOT policy identity or a security
# equivalence claim; the guest agent must establish its own policy hash/state,
# and k8s-apparmor.bats must independently prove guest enforcement.
set -euo pipefail

ci_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${ci_dir}/../../.." && pwd)"
config_tool="${ci_dir}/guest-apparmor-ci-config.py"
install_root=/opt/kata
state=/run/kata-guest-apparmor-ci
profile=kata-apparmor-test

die() { echo "ERROR: $*" >&2; exit 1; }

require_disposable_runner() {
	[[ "${GITHUB_ACTIONS:-}" == true && "${RUNNER_ENVIRONMENT:-}" == github-hosted &&
		"${KATA_GUEST_APPARMOR_TEST:-}" == yes && "${EUID}" -eq 0 ]] ||
		die "host changes require root, a disposable GitHub-hosted runner and KATA_GUEST_APPARMOR_TEST=yes"
	case "${KATA_HYPERVISOR:-}" in
		qemu) defaults="${install_root}/share/defaults/kata-containers" ;;
		qemu-runtime-rs) defaults="${install_root}/share/defaults/kata-containers/runtime-rs" ;;
		*) die "only qemu and qemu-runtime-rs are supported" ;;
	esac
	dropin="${defaults}/runtimes/${KATA_HYPERVISOR}/config.d/99-guest-apparmor-ci.toml"
}

verify_image() (
	local image="$1" variant="$2" filesystem="$3"
	local loop_device="" scratch actual_fs mounted=no result=0
	local root_hash="" salt="" data_blocks="" data_block_size="" hash_block_size="" pair
	local -a fields
	scratch="$(mktemp -d /run/kata-apparmor-image.XXXXXX)"
	# All mounts live in the private namespace. Detach only our read-only loop.
	trap 'result=$?; if [[ "${mounted}" == yes ]]; then umount "${scratch}/root" || result=1; fi;
		if [[ -n "${loop_device}" ]]; then losetup --detach "${loop_device}" || result=1; fi;
		rmdir "${scratch}/root" "${scratch}" || result=1; exit "${result}"' EXIT
	mkdir "${scratch}/root"
	loop_device="$(losetup --find --show --read-only --partscan "${image}")"
	udevadm settle
	[[ -b "${loop_device}p1" && -b "${loop_device}p2" ]] || die "missing measured data/hash partitions: ${image}"
	actual_fs="$(blkid --probe --match-tag TYPE --output value "${loop_device}p1")"
	[[ "${actual_fs}" == "${filesystem}" ]] || die "image filesystem ${actual_fs} disagrees with configured ${filesystem}: ${image}"
	echo "Verified filesystem: ${image} p1=${actual_fs} (matches runtime configuration)"
	IFS=, read -r -a fields < "${install_root}/share/kata-containers/root_hash_${variant}.txt"
	for pair in "${fields[@]}"; do
		case "${pair%%=*}" in
			root_hash) root_hash="${pair#*=}" ;;
			salt) salt="${pair#*=}" ;;
			data_blocks) data_blocks="${pair#*=}" ;;
			data_block_size) data_block_size="${pair#*=}" ;;
			hash_block_size) hash_block_size="${pair#*=}" ;;
			*) die "unexpected verity field: ${pair}" ;;
		esac
	done
	veritysetup verify "${loop_device}p1" "${loop_device}p2" "${root_hash}" \
		--no-superblock --salt="${salt}" --data-blocks="${data_blocks}" \
		--data-block-size="${data_block_size}" --hash-block-size="${hash_block_size}"
	if [[ "${filesystem}" == ext4 ]]; then
		# noload prevents journal replay on the read-only base image.
		mount -t ext4 -o ro,noload "${loop_device}p1" "${scratch}/root"
	else
		mount -t erofs -o ro "${loop_device}p1" "${scratch}/root"
	fi
	mounted=yes
	if [[ "${variant}" == base ]]; then
		# Run only in the disposable runner, against the selected built artifact.
		# This checks real dynamic dependencies too, not just parser filenames.
		# shellcheck disable=SC2016 # Expansion must happen inside the guest root.
		chroot "${scratch}/root" /bin/sh -ec '
			case "$(readlink -f /sbin/init)" in
				/lib/systemd/systemd|/usr/lib/systemd/systemd) test -x /sbin/init ;;
				*) echo "base image does not use systemd as init" >&2; exit 1 ;;
			esac
			command -v apparmor_parser
			apparmor_parser --version
			command -v veritysetup
			veritysetup --version
			test -x /usr/bin/kata-agent
			test -x /usr/libexec/kata-extension-mount.sh
			test -x /usr/lib/systemd/system-generators/kata-extension-mount-generator
			test -s /usr/lib/systemd/system/kata-agent.service
			test -s /usr/lib/systemd/system/kata-extension-mount@.service
		'
	else
		# Refuse stale/different artifacts. The host fixture comes from precisely
		# this repository policy tree, never a permissive same-name placeholder.
		local policy_root="${repo_root}/tools/packaging/apparmor-extension/policy" source_file relative
		while IFS= read -r -d '' source_file; do
			relative="${source_file#"${policy_root}/"}"
			cmp -- "${source_file}" "${scratch}/root/${relative}"
		done < <(find "${policy_root}" -type f -print0)
		[[ -s "${scratch}/root/profiles/${profile}" ]] || die "extension lacks the real test profile"
	fi
	sha256sum -- "${image}" "${install_root}/share/kata-containers/root_hash_${variant}.txt"
)

configure() {
	require_disposable_runner
	local dependency
	for dependency in python3 containerd apparmor_parser unshare losetup udevadm blkid veritysetup mount chroot cmp; do
		command -v "${dependency}" >/dev/null || die "missing dependency: ${dependency}"
	done
	[[ -r /sys/kernel/security/apparmor/profiles ]] || die "host CRI AppArmor policy state is unavailable"
	if grep -Eq "^${profile}([[:space:]]|$)" /sys/kernel/security/apparmor/profiles; then
		die "refusing to replace an existing host ${profile} policy"
	fi
	[[ ! -e "${state}" && ! -L "${state}" ]] || die "CI fixture state already exists"
	# Validate the actual loaded CRI imports, including containerd v2/v3 paths.
	containerd --config /etc/containerd/config.toml config dump | \
		python3 "${config_tool}" check-cri "${install_root}" "${KATA_HYPERVISOR}"
	mkdir -m 0700 "${state}"
	python3 "${config_tool}" render "${install_root}" "${KATA_HYPERVISOR}" "${dropin}"
	sha256sum -- "${dropin}" > "${state}/dropin.sha256"
	# No local/remote deployment, cluster access, or host mount-namespace changes.
	unshare --mount --propagation private bash "${BASH_SOURCE[0]}" verify-images
	cp -a -- "${repo_root}/tools/packaging/apparmor-extension/policy" "${state}/policy"
	local fixture="${state}/policy/profiles/${profile}"
	cmp -- "${repo_root}/tools/packaging/apparmor-extension/policy/profiles/${profile}" "${fixture}"
	apparmor_parser --config-file=/dev/null -Q -K -I "${state}/policy" -b "${state}/policy" "${fixture}"
	# Claim ownership only after the pre-existing-profile check, before loading,
	# so even a partially successful parser invocation gets cleaned up.
	touch "${state}/profile-owned"
	apparmor_parser --config-file=/dev/null -a -K -I "${state}/policy" -b "${state}/policy" "${fixture}"
	grep -Fx "${profile} (enforce)" /sys/kernel/security/apparmor/profiles
	sha256sum -- "${fixture}"
	echo "Temporary host CRI fixture loaded; guest enforcement and guest policy identity still require BATS evidence."
}

cleanup() {
	require_disposable_runner
	if [[ ! -d "${state}" ]]; then
		echo "No AppArmor CI fixture state was created."
		return
	fi
	[[ ! -L "${state}" && "$(readlink -f -- "${state}")" == /run/kata-guest-apparmor-ci ]] || die "unsafe CI state path"
	if [[ -f "${state}/profile-owned" ]]; then
		if grep -Eq "^${profile}([[:space:]]|$)" /sys/kernel/security/apparmor/profiles; then
			apparmor_parser --config-file=/dev/null -R -K -I "${state}/policy" -b "${state}/policy" "${state}/policy/profiles/${profile}"
		fi
		if grep -Eq "^${profile}([[:space:]]|$)" /sys/kernel/security/apparmor/profiles; then
			die "temporary host profile remains loaded"
		fi
		rm -- "${state}/profile-owned"
	fi
	if [[ -f "${state}/dropin.sha256" ]]; then
		# Preserve any intervening edit instead of deleting another writer's work.
		sha256sum --check --strict "${state}/dropin.sha256"
		rm -- "${dropin}" "${state}/dropin.sha256"
	fi
	# Exact, fixed CI state directory, created above, never an install/repo root.
	rm -r --one-file-system -- "${state}"
}

case "${1:-}" in
	configure) configure ;;
	cleanup) cleanup ;;
	verify-images)
		require_disposable_runner
		[[ "$(readlink /proc/self/ns/mnt)" != "$(readlink /proc/1/ns/mnt)" ]] || die "image checks require a private mount namespace"
		base_filesystem="$(python3 "${config_tool}" rootfs-type "${install_root}" "${KATA_HYPERVISOR}")"
		verify_image "${install_root}/share/kata-containers/kata-containers.img" base "${base_filesystem}"
		verify_image "${install_root}/share/kata-containers/kata-containers-apparmor-extension.img" apparmor-extension erofs
		;;
	*) die "usage: $0 {configure|cleanup|verify-images}; CI only" ;;
esac
