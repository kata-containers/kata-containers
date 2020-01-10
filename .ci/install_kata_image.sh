#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

function handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

cidir=$(dirname "$0")
cidir=$(realpath "${cidir}")

source /etc/os-release || source /usr/lib/os-release
source "${cidir}/lib.sh"

ARCH="$(${cidir}/kata-arch.sh -d)"

AGENT_INIT=${AGENT_INIT:-no}
TEST_INITRD=${TEST_INITRD:-no}

PREFIX=${PREFIX:-/usr}
IMAGE_DIR=${PREFIX}/share/kata-containers
IMG_LINK_NAME="kata-containers.img"
INITRD_LINK_NAME="kata-containers-initrd.img"

if [ "${TEST_INITRD}" == "no" ]; then
	OSBUILDER_YAML_INSTALL_NAME="osbuilder-image.yaml"
	LINK_PATH="${IMAGE_DIR}/${IMG_LINK_NAME}"
	IMG_TYPE="image"
else
	OSBUILDER_YAML_INSTALL_NAME="osbuilder-initrd.yaml"
	LINK_PATH="${IMAGE_DIR}/${INITRD_LINK_NAME}"
	IMG_TYPE="initrd"
fi

IMAGE_OS_KEY="assets.${IMG_TYPE}.architecture.$(uname -m).name"
IMAGE_OS_VERSION_KEY="assets.${IMG_TYPE}.architecture.$(uname -m).version"

agent_path="${GOPATH}/src/github.com/kata-containers/agent"
osbuilder_repo="github.com/kata-containers/osbuilder"
osbuilder_path="${GOPATH}/src/${osbuilder_repo}"
latest_build_url="${jenkins_url}/job/image-nightly-$(uname -m)/${cached_artifacts_path}"
tag="${1:-""}"

install_ci_cache_image() {
	type=${1}
	check_not_empty "$type" "image type not provided"
	info "Install pre-built ${type}"
	local image_name=$(curl -fsL "${latest_build_url}/latest-${type}")
	sudo mkdir -p "${IMAGE_DIR}"
	pushd "${IMAGE_DIR}" >/dev/null
	local image_path=$(readlink -f "${IMAGE_DIR}/${image_name}")

	sudo -E curl -fsOL "${latest_build_url}/${type}-tarball.sha256sum"
	sudo -E curl -fsL "${latest_build_url}/${image_name}.tar.xz" -o "${image_path}.tar.xz"
	sudo sha256sum -c "${type}-tarball.sha256sum"

	sudo -E curl -fsOL "${latest_build_url}/sha256sum-${type}"
	sudo tar xfv "${image_path}.tar.xz"
	sudo sha256sum -c "sha256sum-${type}"

	sudo -E ln -sf "${image_path}" "${LINK_PATH}"
	sudo -E curl -fsL "${latest_build_url}/${OSBUILDER_YAML_INSTALL_NAME}" -o "${IMAGE_DIR}/${OSBUILDER_YAML_INSTALL_NAME}"

	popd >/dev/null

	if [ ! -L "${LINK_PATH}" ]; then
		echo "Link path not installed: ${LINK_PATH}"
		false
	fi

	if [ ! -f "$(readlink ${LINK_PATH})" ]; then
		echo "Link to ${LINK_PATH} is broken"
		false
	fi
}

check_not_empty() {
	value=${1:-}
	msg=${2:-}
	if [ -z "${value}" ]; then
		echo "${msg}"
		false
	fi
}

build_image() {
	image_output=${1}
	distro=${2}
	os_version=${3}
	agent_commit=${4}

	check_not_empty "$image_output" "Missing image"
	check_not_empty "$distro" "Missing distro"
	check_not_empty "$os_version" "Missing os version"
	check_not_empty "$agent_commit" "Missing agent commit"

	pushd "${osbuilder_path}" >/dev/null

	readonly ROOTFS_DIR="${PWD}/rootfs"
	export ROOTFS_DIR
	sudo rm -rf "${ROOTFS_DIR}"

	echo "Set runtime as default runtime to build the image"
	bash "${cidir}/../cmd/container-manager/manage_ctr_mgr.sh" docker configure -r runc -f

	sudo -E AGENT_INIT="${AGENT_INIT}" AGENT_VERSION="${agent_commit}" \
		GOPATH="$GOPATH" USE_DOCKER=true OS_VERSION=${os_version} ./rootfs-builder/rootfs.sh "${distro}"

	# Build the image
	if [ "${TEST_INITRD}" == "no" ]; then
		sudo -E AGENT_INIT="${AGENT_INIT}" USE_DOCKER=true ./image-builder/image_builder.sh "$ROOTFS_DIR"
		local image_name="kata-containers.img"

	else
		sudo -E AGENT_INIT="${AGENT_INIT}" USE_DOCKER=true ./initrd-builder/initrd_builder.sh "$ROOTFS_DIR"
		local image_name="kata-containers-initrd.img"
	fi

	sudo install -o root -g root -m 0640 -D ${image_name} "${IMAGE_DIR}/${image_output}"
	sudo install -o root -g root -m 0640 -D "${ROOTFS_DIR}/var/lib/osbuilder/osbuilder.yaml" "${IMAGE_DIR}/${OSBUILDER_YAML_INSTALL_NAME}"
	(cd /usr/share/kata-containers && sudo ln -sf "${IMAGE_DIR}/${image_output}" "${LINK_PATH}")

	popd >/dev/null
}

#Load specific configure file
if [ -f "${cidir}/${ARCH}/lib_kata_image_${ARCH}.sh" ]; then
	source "${cidir}/${ARCH}/lib_kata_image_${ARCH}.sh"
fi

get_dependencies() {
	info "Pull and install agent on host"
	bash -f "${cidir}/install_agent.sh"
	go get -d "${osbuilder_repo}" || true
	[ -z "${tag}" ] || git -C "${osbuilder_path}" checkout -b "${tag}" "${tag}"
}

main() {
	get_dependencies
	local os_version=$(get_version "${IMAGE_OS_VERSION_KEY}")
	local osbuilder_distro=$(get_version "${IMAGE_OS_KEY}")

	if [ "${osbuilder_distro}" == "clearlinux" ] && [ "${os_version}" == "latest" ]; then
		os_version=$(curl -fLs https://download.clearlinux.org/latest)
	fi

	local agent_commit=$(git --work-tree="${agent_path}" --git-dir="${agent_path}/.git" log --format=%h -1 HEAD)
	local osbuilder_commit=$(git --work-tree="${osbuilder_path}" --git-dir="${osbuilder_path}/.git" log --format=%h -1 HEAD)

	image_output="kata-containers-${osbuilder_distro}-${os_version}-osbuilder-${osbuilder_commit}-agent-${agent_commit}"

	if [ "${TEST_INITRD}" == "no" ]; then
		image_output="${image_output}.img"
		type="image"
	else
		image_output="${image_output}.initrd"
		type="initrd"
	fi

	latest_file="latest-${type}"
	info "Image to generate: ${image_output}"

	last_build_image_version=$(curl -fsL "${latest_build_url}/${latest_file}") ||
		last_build_image_version="error-latest-cached-imaget-not-found"

	info "Latest cached image: ${last_build_image_version}"

	if [ "$image_output" == "$last_build_image_version" ]; then
		info "Cached image is same to be generated"
		if ! install_ci_cache_image "${type}"; then
			info "failed to install cached image, trying to build from source"
			build_image "${image_output}" "${osbuilder_distro}" "${os_version}" "${agent_commit}"
		fi
	else
		build_image "${image_output}" "${osbuilder_distro}" "${os_version}" "${agent_commit}"
	fi

	if [ ! -L "${LINK_PATH}" ]; then
		die "Link path not installed: ${LINK_PATH}"
	fi

	if [ ! -f "$(readlink ${LINK_PATH})" ]; then
		die "Link to ${LINK_PATH} is broken"
	fi
}

main $@
