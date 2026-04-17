#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Harden APT for CI: retries and timeouts apply on all architectures. On GitHub
# Actions, amd64 images only: rewrite archive.ubuntu.com and security.ubuntu.com
# to azure.archive.ubuntu.com (typical x86_64 GitHub-hosted runners). Other
# arches and ports.ubuntu.com URLs are left unchanged.
#
# Usage:
#   apt-ci-tune.sh
#       Default: write apt CI config and optional Azure mirror rewrite (amd64 only).
#   apt-ci-tune.sh --mirrors-rewrite
#       Rewrite mirrors only (after new .list lines; amd64 only).

set -euo pipefail

write_apt_ci_conf() {
	printf '%s\n' \
		'Acquire::Retries "5";' \
		'Acquire::http::Timeout "120";' \
		'Acquire::https::Timeout "120";' \
		> /etc/apt/apt.conf.d/99kata-ci
}

# Debian architecture for this image (dpkg is present in Ubuntu base images).
kata_deb_arch() {
	local deb_arch
	if deb_arch=$(dpkg --print-architecture 2>/dev/null) && [[ -n "${deb_arch}" ]]; then
		echo "${deb_arch}"
		return
	fi
	case "$(uname -m)" in
		x86_64) echo amd64 ;;
		i686 | i586) echo i386 ;;
		aarch64) echo arm64 ;;
		armv7l) echo armhf ;;
		ppc64le | powerpc64le) echo ppc64el ;;
		riscv64) echo riscv64 ;;
		s390x) echo s390x ;;
		*) echo amd64 ;;
	esac
}

# Azure CDN for archive/security on amd64 GHA only.
rewrite_ubuntu_mirrors_for_gha() {
	if [[ "${GITHUB_ACTIONS:-false}" != "true" ]]; then
		return 0
	fi
	if [[ "$(kata_deb_arch)" != "amd64" ]]; then
		return 0
	fi

	local azure_main="azure.archive.ubuntu.com/ubuntu"
	local mirrors=(
		"archive.ubuntu.com/ubuntu:${azure_main}"
		"security.ubuntu.com/ubuntu:${azure_main}"
	)
	local sed_args=() m old new

	for m in "${mirrors[@]}"; do
		old="${m%%:*}"
		new="${m#*:}"
		sed_args+=(-e "s|http://${old}|http://${new}|g")
		sed_args+=(-e "s|https://${old}|https://${new}|g")
	done

	find /etc/apt -type f \( -name '*.list' -o -name '*.sources' \) \
		-exec sed -i "${sed_args[@]}" {} + 2>/dev/null || true
}

case "${1:-}" in
	--mirrors-rewrite)
		rewrite_ubuntu_mirrors_for_gha
		;;
	"")
		write_apt_ci_conf
		rewrite_ubuntu_mirrors_for_gha
		;;
	*)
		echo "usage: ${0##*/} [--mirrors-rewrite]" >&2
		exit 1
		;;
esac
