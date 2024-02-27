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

KATA_DEPLOY_IMAGE_TAGS="${KATA_DEPLOY_IMAGE_TAGS:-}"
IFS=' ' read -a IMAGE_TAGS <<< "${KATA_DEPLOY_IMAGE_TAGS}"
KATA_DEPLOY_REGISTRIES="${KATA_DEPLOY_REGISTRIES:-}"
IFS=' ' read -a REGISTRIES <<< "${KATA_DEPLOY_REGISTRIES}"
GH_TOKEN="${GH_TOKEN:-}"
ARCHITECTURE="${ARCHITECTURE:-}"
KATA_STATIC_TARBALL="${KATA_STATIC_TARBALL:-}"
RELEASE_TYPE="${RELEASE_TYPE:-minor}"

function _die()
{
	echo >&2 "ERROR: $*"
	exit 1
}

function _check_required_env_var()
{
	local env_var

	case ${1} in
		RELEASE_VERSION) env_var="${RELEASE_VERSION}" ;;
		GH_TOKEN) env_var="${GH_TOKEN}" ;;
		ARCHITECTURE) env_var="${ARCHITECTURE}" ;;
		KATA_STATIC_TARBALL) env_var="${KATA_STATIC_TARBALL}" ;;
		KATA_DEPLOY_IMAGE_TAGS) env_var="${KATA_DEPLOY_IMAGE_TAGS}" ;;
		KATA_DEPLOY_REGISTRIES) env_var="${KATA_DEPLOY_REGISTRIES}" ;;
		*) >&2 _die "Invalid environment variable \"${1}\"" ;;
	esac

	[ -z "${env_var}" ] && \
		_die "\"${1}\" environment variable is required but was not set"

	return 0
}

function _next_release_version()
{
	local current_release=$(cat "${repo_root_dir}/VERSION")
	local current_major
	local current_everything_else
	local next_major
	local next_minor

	IFS="." read current_major current_minor current_everything_else <<< ${current_release}

	case ${RELEASE_TYPE} in
		major)
			next_major=$(expr $current_major + 1)
			next_minor=0
			;;
		minor)
			next_major=${current_major}
			# TODO: As we're moving from an alpha release to the
			# new scheme, this check is needed for the very first
			# release, after that it can be dropped and only the
			# else part can be kept.
			if grep -qE "alpha|rc" <<< ${current_everything_else}; then
				next_minor=${current_minor}
			else
				next_minor=$(expr $current_minor + 1)
			fi
			;;
		*)
			_die "${RELEASE_TYPE} is not a valid release type, it must be: major or minor"
			;;
	esac

	next_release_number="${next_major}.${next_minor}.0"
	echo "test-${next_release_number}"
}

function _update_version_file()
{
	_check_required_env_var "RELEASE_VERSION"

	git config user.email "katacontainersbot@gmail.com"
	git config user.name "Kata Containers Bot"

	echo "${RELEASE_VERSION}" > "${repo_root_dir}/VERSION"
	git diff
	git add "${repo_root_dir}/VERSION"
	git commit -s -m "release: Kata Containers ${RELEASE_VERSION}"
	git push
}

function _create_new_release()
{
	_check_required_env_var "RELEASE_VERSION"
	_check_required_env_var "GH_TOKEN"

	gh release create ${RELEASE_VERSION} --generate-notes --title "Kata Containers ${RELEASE_VERSION}"
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

	RELEASE_VERSION="$(_next_release_version)"

	new_tarball_name="kata-static-${RELEASE_VERSION}-${ARCHITECTURE}.tar.xz"
	mv ${KATA_STATIC_TARBALL} "${new_tarball_name}"
	echo "uploading asset '${new_tarball_name}' (${ARCHITECTURE}) for tag: ${RELEASE_VERSION}"
	gh release upload "${RELEASE_VERSION}" "${new_tarball_name}"
}

function _upload_versions_yaml_file()
{
	RELEASE_VERSION="$(_next_release_version)"

	versions_file="kata-containers-${RELEASE_VERSION}-versions.yaml"
	cp "${repo_root_dir}/versions.yaml" ${versions_file}
	gh release upload "${RELEASE_VERSION}" "${versions_file}"
}

function _upload_vendored_code_tarball()
{
	_check_required_env_var "GH_TOKEN"

	RELEASE_VERSION="$(_next_release_version)"

	vendored_code_tarball="kata-containers-${RELEASE_VERSION}-vendor.tar.gz"
	bash -c "${repo_root_dir}/tools/packaging/release/generate_vendor.sh ${vendored_code_tarball}"
	gh release upload "${RELEASE_VERSION}" "${vendored_code_tarball}"
}

function _upload_libseccomp_tarball()
{
	_check_required_env_var "GH_TOKEN"

	RELEASE_VERSION="$(_next_release_version)"

	GOPATH=${HOME}/go ./ci/install_yq.sh

	versions_yaml="versions.yaml"
	version=$(${HOME}/go/bin/yq read ${versions_yaml} "externals.libseccomp.version")
	repo_url=$(${HOME}/go/bin/yq read ${versions_yaml} "externals.libseccomp.url")
	download_url="${repo_url}releases/download/v${version}"
	tarball="libseccomp-${version}.tar.gz"
	asc="${tarball}.asc"
	curl -sSLO "${download_url}/${tarball}"
	curl -sSLO "${download_url}/${asc}"
	gh release upload "${RELEASE_VERSION}" "${tarball}"
	gh release upload "${RELEASE_VERSION}" "${asc}"
}

function main()
{
	action="${1:-}"

	case "${action}" in
		publish-multiarch-manifest) _publish_multiarch_manifest ;;
		update-version-file) _update_version_file ;;
		next-release-version) _next_release_version;;
		create-new-release) _create_new_release ;;
		upload-kata-static-tarball) _upload_kata_static_tarball ;;
		upload-versions-yaml-file) _upload_versions_yaml_file ;;
		upload-vendored-code-tarball) _upload_vendored_code_tarball ;;
		upload-libseccomp-tarball) _upload_libseccomp_tarball ;;
		*) >&2 _die "Invalid argument" ;;
	esac
}

main "$@"
