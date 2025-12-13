#!/usr/bin/env bash
#
# Copyright (c) 2025 Kata Containers Community
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Script to publish kata-deploy payload to container registry.

set -o errexit
set -o nounset
set -o pipefail

DEBUG="${DEBUG:-}"
[[ -n "${DEBUG}" ]] && set -x

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "${script_dir}/.." && pwd)"

function info() {
	echo "[INFO] $*"
}

function die() {
	echo "[ERROR] $*" >&2
	exit 1
}

function publish_payload() {
	local tarball="${1}"
	local registry_repo="${2}"
	local tag="${3}"

	info "Publishing payload to ${registry_repo}:${tag}"
	info "Using tarball: ${tarball}"

	"${repo_root_dir}/tools/packaging/kata-deploy/local-build/kata-deploy-build-and-upload-payload.sh" \
		"${tarball}" \
		"${registry_repo}" \
		"${tag}"

	info "Payload published successfully"
}

function main() {
	action="${1:-}"

	case "${action}" in
		publish-payload)
			local tarball="${2:-}"
			local registry_repo="${3:-}"
			local tag="${4:-}"
			[[ -z "${tarball}" ]] && die "tarball path is required"
			[[ -z "${registry_repo}" ]] && die "registry/repo is required"
			[[ -z "${tag}" ]] && die "tag is required"
			publish_payload "${tarball}" "${registry_repo}" "${tag}"
			;;
		*)
			die "Invalid argument: ${action}. Valid actions: publish-payload"
			;;
	esac
}

main "$@"
