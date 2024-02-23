#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

[ -n "${DEBUG:-}" ] && set -o xtrace

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "$this_script_dir/../../../" && pwd)"

IFS=' ' read -a IMAGE_TAGS <<< "${KATA_DEPLOY_IMAGE_TAGS:-}"
IFS=' ' read -a REGISTRIES <<< "${KATA_DEPLOY_REGISTRIES:-}"
GH_TOKEN="${GH_TOKEN:-}"
ARCHITECTURE="${ARCHITECURE:-}"
KATA_STATIC_TARBALL="${KATA_STATIC_TARBALL:-}"
RELEASE_VERSION="${RELEASE_VERSION:-}"

function _die()
{
	echo >&2 "ERROR: $*"
	exit 1
}

function _check_required_env_var()
{
	local env_var

	case ${1} in
		KATA_DEPLOY_IMAGE_TAGS) env_var="${KATA_DEPLOY_IMAGE_TAGS}" ;;
		KATA_DEPLOY_REGISTRIES) env_var="${KATA_DEPLOY_REGISTRIES}" ;;
		*) >&2 _die "Invalid environment variable \"${1}\"" ;;
	esac

	[ -z "${env_var}" ] && \
		_die "\"${1}\" environment variable is required but was not set"
}

function _publish_multiarch_manifest()
{
	_check_required_env_var "KATA_DEPLOY_IMAGE_TAGS"
	_check_required_env_var "KATA_DEPLOY_REGISTRIES"

	for registry in ${REGISTRIES[@]}; do
		for tag in ${IMAGE_TAGS[@]}; do
			docker manifest create ${registry}:${tag} \
				--amend ${registry}:${tag}-amd64 \
				--amend ${registry}:${tag}-arm64 \
				--amend ${registry}:${tag}-s390x \
				--amend ${registry}:${tag}-ppc64le

			docker manifest push ${registry}:${tag}
		done
	done
}

function _upload_kata_static_tarball()
{
	_check_required_env_var "GH_TOKEN"
	_check_required_env_var "ARCHITECTURE"
	_check_required_env_var "KATA_STATIC_TARBALL"

	[ -z "${RELEASE_VERSION}" ] && RELEASE_VERSION=$(cat "${repo_root_dir}/VERSION")

	new_tarball_name="kata-static-${RELEASE_VERSION}-${ARCHITECTURE}.tar.xz"
	mv ${KATA_STATIC_TARBALL} "${new_tarball_name}"
	echo "uploading asset '${new_tarball_name}' (${ARCHITECTURE}) for tag: ${RELEASE_VERSION}"
	gh release upload "${RELEASE_VERSION}" "${new_tarball_name}"
}

function _upload_versions_yaml_file()
{
	[ -z "${RELEASE_VERSION}" ] && RELEASE_VERSION=$(cat "${repo_root_dir}/VERSION")

	versions_file="kata-containers-${RELEASE_VERSION}-versions.yaml"
	cp "${repo_root_dir}/versions.yaml" ${versions_file}
	gh release upload "${RELEASE_VERSION}" "${versions_file}"
}

function main()
{
	action="${1:-}"

	case "${action}" in
		publish-multiarch-manifest) _publish_multiarch_manifest ;;
		upload-kata-static-tarball) _upload_kata_static_tarball ;;
		upload-versions-yaml-file) _upload_versions_yaml_file ;;
		*) >&2 _die "Invalid argument" ;;
	esac
}

main "$@"
