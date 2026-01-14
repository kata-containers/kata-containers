#!/bin/bash
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This script populates the ORAS cache with gperf and busybox tarballs.
# It should be run when versions change in versions.yaml or to initially
# populate the cache.
#
# Requirements:
# - ARTEFACT_REGISTRY_USERNAME and ARTEFACT_REGISTRY_PASSWORD must be set
# - Or be already logged into the registry
#
# Usage:
#   ./populate-oras-tarball-cache.sh [--dry-run] [gperf|busybox|all]
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source the main helper (which sources lib.sh)
source "${script_dir}/download-with-oras-cache.sh"

DRY_RUN="${DRY_RUN:-no}"
FORCE_PUSH="${FORCE_PUSH:-no}"

usage() {
	cat <<EOF
Usage: $0 [OPTIONS] [COMPONENT...]

Populate the ORAS cache with tarballs for gperf and/or busybox.

Components:
  gperf      - Cache gperf tarball
  busybox    - Cache busybox tarball
  all        - Cache all tarballs (default)

Options:
  --dry-run     Show what would be done without actually pushing
  --force       Push even if the version already exists in cache
  -h, --help    Show this help message

Environment variables:
  ARTEFACT_REGISTRY           - Registry to use (default: ghcr.io)
  ARTEFACT_REPOSITORY         - Repository org/path (default: kata-containers)
  ARTEFACT_REGISTRY_USERNAME  - Username for registry authentication
  ARTEFACT_REGISTRY_PASSWORD  - Password for registry authentication

Examples:
  # Populate cache for all tarballs
  $0 all

  # Only cache gperf
  $0 gperf

  # Dry run to see what would happen
  $0 --dry-run all
EOF
	exit "${1:-0}"
}

check_version_in_cache() {
	local artifact_name="$1"
	local version="$2"

	local oci_image="${ARTEFACT_REGISTRY:?}/${ARTEFACT_REPOSITORY:?}/cached-tarballs/${artifact_name}:${version}"

	info "Checking if ${artifact_name} version ${version} exists in cache..."

	if oras manifest fetch "${oci_image}" &>/dev/null; then
		info "Version ${version} already exists in cache"
		return 0
	fi

	info "Version ${version} not found in cache"
	return 1
}

# Generic function to cache a component
cache_component() {
	local component="$1"

	local version
	version=$(get_from_kata_deps ".externals.${component}.version")
	local base_url
	base_url=$(get_from_kata_deps ".externals.${component}.url")

	info "=== Caching ${component} version ${version} ==="

	if [[ "${FORCE_PUSH}" != "yes" ]] && check_version_in_cache "${component}" "${version}"; then
		info "Skipping ${component} - already cached (use --force to override)"
		return 0
	fi

	# Component-specific tarball naming
	local tarball_name tarball_url
	case "${component}" in
		gperf)
			tarball_name="gperf-${version}.tar.gz"
			;;
		busybox)
			tarball_name="busybox-${version}.tar.bz2"
			;;
		*)
			die "Unknown component: ${component}"
			;;
	esac
	tarball_url="${base_url}/${tarball_name}"

	if [[ "${DRY_RUN}" == "yes" ]]; then
		info "[DRY-RUN] Would download ${component} from: ${tarball_url}"
		info "[DRY-RUN] Would push to: ${ARTEFACT_REGISTRY}/${ARTEFACT_REPOSITORY}/cached-tarballs/${component}:${version}"
		return 0
	fi

	local tmpdir
	tmpdir=$(mktemp -d)
	# shellcheck disable=SC2064 # tmpdir is intentionally expanded at trap setup time
	trap "rm -rf ${tmpdir}" EXIT

	info "Downloading ${component} from upstream using ORAS cache helper..."
	export PUSH_TO_REGISTRY="yes"
	local tarball_path
	tarball_path=$(download_component "${component}" "${tmpdir}")

	if [[ ! -f "${tarball_path}" ]]; then
		die "Failed to download ${component}"
	fi

	info "Successfully cached ${component} version ${version}"
}

# Backward compatibility wrappers
cache_gperf() {
	cache_component "gperf"
}

cache_busybox() {
	cache_component "busybox"
}

main() {
	local components=()

	while [[ $# -gt 0 ]]; do
		case "$1" in
			--dry-run)
				DRY_RUN="yes"
				shift
				;;
			--force)
				FORCE_PUSH="yes"
				shift
				;;
			-h|--help)
				usage 0
				;;
			gperf|busybox|all)
				components+=("$1")
				shift
				;;
			*)
				echo "Unknown option: $1"
				usage 1
				;;
		esac
	done

	# Default to all if no components specified
	if [[ ${#components[@]} -eq 0 ]]; then
		components=("all")
	fi

	# Ensure ORAS is installed
	ensure_oras_installed

	# Check credentials unless dry-run
	if [[ "${DRY_RUN}" != "yes" ]]; then
		if [[ -z "${ARTEFACT_REGISTRY_USERNAME:-}" ]] || [[ -z "${ARTEFACT_REGISTRY_PASSWORD:-}" ]]; then
			die "ARTEFACT_REGISTRY_USERNAME and ARTEFACT_REGISTRY_PASSWORD must be set"
		fi
	fi

	for component in "${components[@]}"; do
		case "${component}" in
			gperf)
				cache_gperf
				;;
			busybox)
				cache_busybox
				;;
			all)
				cache_gperf
				cache_busybox
				;;
		esac
	done

	info "=== Cache population complete ==="
}

main "$@"
