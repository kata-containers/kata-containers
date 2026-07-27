#!/usr/bin/env bash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Refuse a payload that would install itself over the host's own configuration.
#
# Component tarballs are merged into kata-static.tar.zst, which is unpacked with
# "tar -C /" (see the install-tarball target), so a member named
# "etc/resolv.conf" replaces the resolver of whatever machine installs the
# payload. Container filesystem exports are how such a member sneaks in:
# "docker export" dumps the empty /etc/{resolv.conf,hosts,hostname,mtab}
# bind-mount placeholders and the /dev, /proc and /sys mount points alongside
# the image content. An empty /etc/resolv.conf leaves the machine without DNS,
# and a persistent CI runner does not recover from that on its own.

set -o errexit
set -o nounset
set -o pipefail

script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_name

# Members that would land on top of host configuration.
readonly forbidden_paths=(
	.dockerenv
	etc/hostname
	etc/hosts
	etc/mtab
	etc/resolv.conf
)

# Members that would land on top of a host mount point. Kata payloads carry
# guest content as images and tarballs, never as a live device tree.
readonly forbidden_prefixes=(
	dev/
	proc/
	sys/
)

usage() {
	cat <<-EOF
	Usage: ${script_name} <tarball> [<tarball>...]

	Exits non-zero if a tarball carries a member that would overwrite the
	host's own configuration when the payload is extracted at /.
	EOF
}

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

list_members() {
	local tarball="$1"

	case "${tarball}" in
		*.zst | *.zstd) tar --zstd -tf "${tarball}" ;;
		*) tar -tf "${tarball}" ;;
	esac
}

# Members are named "./etc/resolv.conf", "etc/resolv.conf" or "etc/" depending
# on how the tarball was created, so compare a single normalised form.
normalize_member() {
	local member="$1"

	member="${member#./}"
	member="${member#/}"
	echo "${member%/}"
}

member_is_forbidden() {
	local member="$1"
	local forbidden

	for forbidden in "${forbidden_paths[@]}"; do
		if [[ "${member}" == "${forbidden}" ]]; then
			return 0
		fi
	done

	for forbidden in "${forbidden_prefixes[@]}"; do
		# The bare mount point counts too: normalisation dropped its slash.
		if [[ "${member}" == "${forbidden}"* || "${member}" == "${forbidden%/}" ]]; then
			return 0
		fi
	done

	return 1
}

check_tarball() {
	local tarball="$1"
	local member normalized
	local -a offenders=()

	[[ -f "${tarball}" ]] || die "${tarball}: no such tarball"

	while IFS= read -r member; do
		normalized="$(normalize_member "${member}")"
		[[ -n "${normalized}" ]] || continue
		if member_is_forbidden "${normalized}"; then
			offenders+=("${member}")
		fi
	done < <(list_members "${tarball}")

	if [[ "${#offenders[@]}" -gt 0 ]]; then
		echo >&2 "ERROR: ${tarball} carries members that overwrite host configuration:"
		printf >&2 '  %s\n' "${offenders[@]}"
		die "exclude them where the payload is assembled"
	fi
}

main() {
	if [[ $# -eq 0 ]]; then
		usage
		exit 1
	fi

	local tarball
	for tarball in "$@"; do
		check_tarball "${tarball}"
	done
}

main "$@"
