#!/usr/bin/env bash
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0

# Exercise the real Make entry points and release manifests without building
# images, contacting a registry, or creating/replacing build outputs.
set -euo pipefail
export LC_ALL=C

test_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${test_dir}/../../../../.." && pwd)"
inputs_script="${test_dir}/../kata-release-tarball-inputs.sh"
extension=rootfs-image-apparmor-extension
extension_tarball="kata-static-${extension}.tar.zst"
checks=0

die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

dry_run() {
	# Recursive $(MAKE) recipes inherit -n; the final Docker wrapper is printed
	# only. Clear inherited Make flags so quiet mode cannot hide that command.
	env -u MAKEFLAGS -u MFLAGS -u MAKEOVERRIDES \
		make --no-print-directory -n -C "${repo_root}" "ARCH=$1" DEPS= "$2"
}

for arch in x86_64 aarch64; do
	output="$(dry_run "${arch}" "${extension}-tarball")" || \
		die "${arch}: extension Make entry is unreachable"
	count="$(grep -Ec -- "--build=${extension}([[:space:]]|$)" <<< "${output}" || true)"
	[[ "${count}" -eq 1 ]] || die "${arch}: expected exactly one extension build command, got ${count}"
	if grep -Eq -- '--build=agent([[:space:]]|$)|kata-deploy-copy-libseccomp-installer\.sh' <<< "${output}"; then
		die "${arch}: policy-only extension unexpectedly depends on Agent build inputs"
	fi
	printf 'ok %s - %s extension Make entry reaches the Docker wrapper in dry-run only\n' "$((++checks))" "${arch}"

	output="$(dry_run "${arch}" agent-tarball)" || die "${arch}: Agent Make entry is unreachable"
	# Agent's existing input-generation prerequisite must survive DEPS=, and
	# must precede the actual Agent build command.
	awk '
		/kata-deploy-copy-libseccomp-installer\.sh "agent"/ { prepared = 1 }
		/--build=agent([[:space:]]|$)/ { if (!prepared) exit 1; built = 1 }
		END { if (!built) exit 1 }
	' <<< "${output}" || die "${arch}: Agent build inputs are missing or prepared too late"
	printf 'ok %s - %s Agent entry prepares its inputs before building\n' "$((++checks))" "${arch}"
done

for arch in amd64 x86_64 arm64 aarch64; do
	for bundle in kata-static kata-go-static; do
		inputs="$(bash "${inputs_script}" "${bundle}" "${arch}")" || \
			die "${bundle}/${arch}: cannot read the release manifest"
		for component in kata-static-rootfs-image.tar.zst "${extension_tarball}"; do
			count="$(grep -Fxc -- "${component}" <<< "${inputs}" || true)"
			[[ "${count}" -eq 1 ]] || \
				die "${bundle}/${arch}: expected exactly one ${component}, got ${count}"
		done
		printf 'ok %s - %s/%s ships the base and AppArmor extension exactly once\n' "$((++checks))" "${bundle}" "${arch}"
	done
done

printf 'PASS: %s AppArmor build-entry and release-manifest checks\n' "${checks}"
