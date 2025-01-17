#!/usr/bin/env bash
#
# Copyright (c) 2018-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname "$(readlink -f "$0")")
kata_dir=$(realpath "${script_dir}/../../../../")
kata_deploy_create="${script_dir}/kata-deploy-binaries.sh"
uid=$(id -u ${USER})
gid=$(id -g ${USER})
http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"

ARCH=${ARCH:-$(uname -m)}
CROSS_BUILD=
BUILDX=""
PLATFORM=""
TARGET_ARCH=${TARGET_ARCH:-$(uname -m)}
[ "$(uname -m)" != "${TARGET_ARCH}" ] && CROSS_BUILD=true

[ "${TARGET_ARCH}" == "aarch64" ] && TARGET_ARCH=arm64

# used for cross build
TARGET_OS=${TARGET_OS:-linux}
TARGET_ARCH=${TARGET_ARCH:-$ARCH}

# We've seen issues related to the /home/runner/.docker/buildx/activity/default file
# constantly being with the wrong permissions.
# Let's just remove the file before we build.
rm -f $HOME/.docker/buildx/activity/default

[ "${CROSS_BUILD}" == "true" ] && BUILDX="buildx" && PLATFORM="--platform=${TARGET_OS}/${TARGET_ARCH}"
if [ "${CROSS_BUILD}" == "true" ]; then
       # check if the current docker support docker buildx
       docker buildx ls > /dev/null 2>&1 || true
       [ $? != 0 ] && echo "no docker buildx support, please upgrad your docker" && exit 1
       # check if docker buildx support target_arch, if not install it
       r=$(docker buildx ls | grep "${TARGET_ARCH}" || true)
       [ -z "$r" ] && sudo docker run --privileged --rm tonistiigi/binfmt --install ${TARGET_ARCH}
fi

if [ "${script_dir}" != "${PWD}" ]; then
	ln -sf "${script_dir}/build" "${PWD}/build"
fi

# This is the gid of the "docker" group on host. In case of docker in docker builds
# for some of the targets (clh builds from source), the nested container user needs to
# be part of this group.
docker_gid=$(getent group docker | cut -d: -f3 || { echo >&2 "Missing docker group, docker needs to be installed" && false; })

# If docker gid is the effective group id of the user, do not pass it as
# an additional group.
if [ ${docker_gid} == ${gid} ]; then
	docker_gid=""
fi

remove_dot_docker_dir=false
if [ ! -d "$HOME/.docker" ]; then
	mkdir $HOME/.docker
	remove_dot_docker_dir=true
fi

"${script_dir}"/kata-deploy-copy-yq-installer.sh
docker build -q -t build-kata-deploy \
	--build-arg IMG_USER="${USER}" \
	--build-arg UID=${uid} \
	--build-arg GID=${gid} \
	--build-arg http_proxy="${http_proxy}" \
	--build-arg https_proxy="${https_proxy}" \
	--build-arg HOST_DOCKER_GID=${docker_gid} \
	--build-arg ARCH="${ARCH}" \
	"${script_dir}/dockerbuild/"

ARTEFACT_REGISTRY="${ARTEFACT_REGISTRY:-}"
ARTEFACT_REPOSITORY="${ARTEFACT_REPOSITORY:-}"
ARTEFACT_REGISTRY_USERNAME="${ARTEFACT_REGISTRY_USERNAME:-}"
ARTEFACT_REGISTRY_PASSWORD="${ARTEFACT_REGISTRY_PASSWORD:-}"
TARGET_BRANCH="${TARGET_BRANCH:-}"
BUILDER_REGISTRY="${BUILDER_REGISTRY:-}"
PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-"no"}"
RELEASE="${RELEASE:-"no"}"
AGENT_CONTAINER_BUILDER="${AGENT_CONTAINER_BUILDER:-}"
COCO_GUEST_COMPONENTS_CONTAINER_BUILDER="${COCO_GUEST_COMPONENTS_CONTAINER_BUILDER:-}"
INITRAMFS_CONTAINER_BUILDER="${INITRAMFS_CONTAINER_BUILDER:-}"
KERNEL_CONTAINER_BUILDER="${KERNEL_CONTAINER_BUILDER:-}"
OVMF_CONTAINER_BUILDER="${OVMF_CONTAINER_BUILDER:-}"
PAUSE_IMAGE_CONTAINER_BUILDER="${PAUSE_IMAGE_CONTAINER_BUILDER:-}"
QEMU_CONTAINER_BUILDER="${QEMU_CONTAINER_BUILDER:-}"
SHIM_V2_CONTAINER_BUILDER="${SHIM_V2_CONTAINER_BUILDER:-}"
TDSHIM_CONTAINER_BUILDER="${TDSHIM_CONTAINER_BUILDER:-}"
TOOLS_CONTAINER_BUILDER="${TOOLS_CONTAINER_BUILDER:-}"
VIRTIOFSD_CONTAINER_BUILDER="${VIRTIOFSD_CONTAINER_BUILDER:-}"
AGENT_INIT="${AGENT_INIT:-no}"
MEASURED_ROOTFS="${MEASURED_ROOTFS:-}"
PULL_TYPE="${PULL_TYPE:-default}"
USE_CACHE="${USE_CACHE:-}"
BUSYBOX_CONF_FILE=${BUSYBOX_CONF_FILE:-}
NVIDIA_GPU_STACK="${NVIDIA_GPU_STACK:-}"

docker run \
	-v $HOME/.docker:/root/.docker \
	-v /var/run/docker.sock:/var/run/docker.sock \
	-v "${kata_dir}:${kata_dir}" \
	--env USER=${USER} \
	--env ARTEFACT_REGISTRY="${ARTEFACT_REGISTRY}" \
	--env ARTEFACT_REPOSITORY="${ARTEFACT_REPOSITORY}" \
	--env ARTEFACT_REGISTRY_USERNAME="${ARTEFACT_REGISTRY_USERNAME}" \
	--env ARTEFACT_REGISTRY_PASSWORD="${ARTEFACT_REGISTRY_PASSWORD}" \
	--env TARGET_BRANCH="${TARGET_BRANCH}" \
	--env RELEASE="${RELEASE}" \
	--env BUILDER_REGISTRY="${BUILDER_REGISTRY}" \
	--env PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY}" \
	--env AGENT_CONTAINER_BUILDER="${AGENT_CONTAINER_BUILDER}" \
	--env COCO_GUEST_COMPONENTS_CONTAINER_BUILDER="${COCO_GUEST_COMPONENTS_CONTAINER_BUILDER}" \
	--env INITRAMFS_CONTAINER_BUILDER="${INITRAMFS_CONTAINER_BUILDER}" \
	--env KERNEL_CONTAINER_BUILDER="${KERNEL_CONTAINER_BUILDER}" \
	--env OVMF_CONTAINER_BUILDER="${OVMF_CONTAINER_BUILDER}" \
	--env PAUSE_IMAGE_CONTAINER_BUILDER="${PAUSE_IMAGE_CONTAINER_BUILDER}" \
	--env QEMU_CONTAINER_BUILDER="${QEMU_CONTAINER_BUILDER}" \
	--env SHIM_V2_CONTAINER_BUILDER="${SHIM_V2_CONTAINER_BUILDER}" \
	--env TDSHIM_CONTAINER_BUILDER="${TDSHIM_CONTAINER_BUILDER}" \
	--env TOOLS_CONTAINER_BUILDER="${TOOLS_CONTAINER_BUILDER}" \
	--env VIRTIOFSD_CONTAINER_BUILDER="${VIRTIOFSD_CONTAINER_BUILDER}" \
	--env AGENT_INIT="${AGENT_INIT}" \
	--env MEASURED_ROOTFS="${MEASURED_ROOTFS}" \
	--env PULL_TYPE="${PULL_TYPE}" \
	--env USE_CACHE="${USE_CACHE}" \
	--env BUSYBOX_CONF_FILE="${BUSYBOX_CONF_FILE}" \
	--env NVIDIA_GPU_STACK="${NVIDIA_GPU_STACK}" \
	--env AA_KBC="${AA_KBC:-}" \
	--env HKD_PATH="$(realpath "${HKD_PATH:-}" 2> /dev/null || true)" \
	--env SE_KERNEL_PARAMS="${SE_KERNEL_PARAMS:-}" \
	--env CROSS_BUILD="${CROSS_BUILD}" \
	--env TARGET_ARCH="${TARGET_ARCH}" \
	--env ARCH="${ARCH}" \
	--rm \
	-w ${script_dir} \
	build-kata-deploy "${kata_deploy_create}" $@

if [ $remove_dot_docker_dir == true ]; then
	rm -rf "$HOME/.docker"
fi
