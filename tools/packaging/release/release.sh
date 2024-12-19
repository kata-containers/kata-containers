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

function _release_version()
{
	cat "${repo_root_dir}/VERSION"
}

function _create_our_own_notes()
{
	GOPATH=${HOME}/go ./ci/install_yq.sh
	export PATH=${HOME}/go/bin:${PATH}

	source "${repo_root_dir}/tools/packaging/scripts/lib.sh"
	libseccomp_version=$(get_from_kata_deps ".externals.libseccomp.version")
	libseccomp_url=$(get_from_kata_deps ".externals.libseccomp.url")

	cat >> /tmp/our_notes_${RELEASE_VERSION} <<EOF
## Survey

Please take the Kata Containers survey:

- https://openinfrafoundation.formstack.com/forms/kata_containers_user_survey

This will help the Kata Containers community understand:

- how you use Kata Containers
- what features and improvements you would like to see in Kata Containers

## Libseccomp Notices
The \`kata-agent\` binaries inside the Kata Containers images provided with this release are
statically linked with the following [GNU LGPL-2.1][lgpl-2.1] licensed libseccomp library.

* [\`libseccomp\`][libseccomp]

The \`kata-agent\` uses the libseccomp v${libseccomp_version} which is not modified from the upstream version.
However, in order to comply with the LGPL-2.1 (§6(a)), we attach the complete source code for the library.

## Kata Containers builder images

* agent (on all its different flavours): $(get_agent_image_name)
* Kernel (on all its different flavours): $(get_kernel_image_name)
* OVMF (on all its different flavours): $(get_ovmf_image_name)
* QEMU (on all its different flavurs): $(get_qemu_image_name)
* shim-v2: $(get_shim_v2_image_name)
* tools: $(get_tools_image_name)
* virtiofsd: $(get_virtiofsd_image_name)

## Installation

Follow the Kata [installation instructions][installation].

[libseccomp]: ${libseccomp_url}
[lgpl-2.1]: https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html
[installation]: https://github.com/kata-containers/kata-containers/blob/${RELEASE_VERSION}/docs/install
EOF

	return 0
}

function _create_new_release()
{
	_check_required_env_var "GH_TOKEN"

	RELEASE_VERSION="$(_release_version)"

	if gh release view ${RELEASE_VERSION}; then
		echo "Release already exists, aborting"
		exit 1
	fi

	_create_our_own_notes

	# This automatically creates the ${RELEASE_VERSION} tag in the repo
	gh release create ${RELEASE_VERSION} \
		--generate-notes --title "Kata Containers ${RELEASE_VERSION}" \
		--notes-file "/tmp/our_notes_${RELEASE_VERSION}" \
		--draft
}

function _publish_release()
{
	_check_required_env_var "GH_TOKEN"

	RELEASE_VERSION="$(_release_version)"

	# Make the release live on GitHub
	gh release edit ${RELEASE_VERSION} \
		--verify-tag \
		--draft=false
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

	RELEASE_VERSION="$(_release_version)"

	new_tarball_name="kata-static-${RELEASE_VERSION}-${ARCHITECTURE}.tar.xz"
	mv ${KATA_STATIC_TARBALL} "${new_tarball_name}"
	echo "uploading asset '${new_tarball_name}' (${ARCHITECTURE}) for tag: ${RELEASE_VERSION}"
	gh release upload "${RELEASE_VERSION}" "${new_tarball_name}"
}

function _upload_versions_yaml_file()
{
	RELEASE_VERSION="$(_release_version)"

	versions_file="kata-containers-${RELEASE_VERSION}-versions.yaml"
	cp "${repo_root_dir}/versions.yaml" ${versions_file}
	gh release upload "${RELEASE_VERSION}" "${versions_file}"
}

function _upload_vendored_code_tarball()
{
	_check_required_env_var "GH_TOKEN"

	RELEASE_VERSION="$(_release_version)"

	vendored_code_tarball="kata-containers-${RELEASE_VERSION}-vendor.tar.gz"
	bash -c "${repo_root_dir}/tools/packaging/release/generate_vendor.sh ${vendored_code_tarball}"
	gh release upload "${RELEASE_VERSION}" "${vendored_code_tarball}"
}

function _upload_libseccomp_tarball()
{
	_check_required_env_var "GH_TOKEN"

	RELEASE_VERSION="$(_release_version)"

	GOPATH=${HOME}/go ./ci/install_yq.sh

	versions_yaml="versions.yaml"
	version=$(${HOME}/go/bin/yq ".externals.libseccomp.version" ${versions_yaml})
	repo_url=$(${HOME}/go/bin/yq ".externals.libseccomp.url" ${versions_yaml})
	download_url="${repo_url}releases/download/v${version}"
	tarball="libseccomp-${version}.tar.gz"
	asc="${tarball}.asc"
	curl -sSLO "${download_url}/${tarball}"
	curl -sSLO "${download_url}/${asc}"
	gh release upload "${RELEASE_VERSION}" "${tarball}"
	gh release upload "${RELEASE_VERSION}" "${asc}"
}

function _upload_helm_chart_tarball()
{
	_check_required_env_var "GH_TOKEN"

	RELEASE_VERSION="$(_release_version)"

	helm package ${repo_root_dir}/tools/packaging/kata-deploy/helm-chart/kata-deploy
	gh release upload "${RELEASE_VERSION}" "kata-deploy-${RELEASE_VERSION}.tgz"
}

function main()
{
	action="${1:-}"

	case "${action}" in
		publish-multiarch-manifest) _publish_multiarch_manifest ;;
		release-version) _release_version;;
		create-new-release) _create_new_release ;;
		upload-kata-static-tarball) _upload_kata_static_tarball ;;
		upload-versions-yaml-file) _upload_versions_yaml_file ;;
		upload-vendored-code-tarball) _upload_vendored_code_tarball ;;
		upload-libseccomp-tarball) _upload_libseccomp_tarball ;;
		upload-helm-chart-tarball) _upload_helm_chart_tarball ;;
		publish-release) _publish_release ;;
		*) >&2 _die "Invalid argument" ;;
	esac
}

main "$@"
