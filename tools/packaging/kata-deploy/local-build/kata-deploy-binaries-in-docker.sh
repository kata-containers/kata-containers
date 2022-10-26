#!/usr/bin/env bash
#
# Copyright (c) 2018-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname "$(readlink -f "$0")")
kata_dir=$(realpath "${script_dir}/../../../../")
kata_deploy_create="${script_dir}/kata-deploy-binaries.sh"
uid=$(id -u ${USER})
gid=$(id -g ${USER})

if [ "${script_dir}" != "${PWD}" ]; then
	ln -sf "${script_dir}/build" "${PWD}/build"
fi

# This is the gid of the "docker" group on host. In case of docker in docker builds
# for some of the targets (clh builds from source), the nested container user needs to
# be part of this group.
docker_gid=$(getent group docker | cut -d: -f3 || { echo >&2 "Missing docker group, probably podman is being used" && echo "${gid}"; })

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

docker build -q -t build-kata-deploy \
	--build-arg IMG_USER="${USER}" \
	--build-arg UID=${uid} \
	--build-arg GID=${gid} \
	--build-arg HOST_DOCKER_GID=${docker_gid} \
	"${script_dir}/dockerbuild/"

docker run \
	--privileged \
	-v $HOME/.docker:/root/.docker \
	-v /var/run/docker.sock:/var/run/docker.sock \
	--user ${uid}:${gid} \
	--env CI="${CI:-}" \
	--env USER=${USER} \
	--env SKOPEO="${SKOPEO:-}" \
	--env UMOCI="${UMOCI:-}" \
	--env AA_KBC="${AA_KBC:-}" \
	--env KATA_BUILD_CC="${KATA_BUILD_CC:-}" \
	--env INCLUDE_ROOTFS="$(realpath "${INCLUDE_ROOTFS:-}" 2> /dev/null || true)" \
	--env PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-"no"}" \
	--env INITRAMFS_CONTAINER_BUILDER="${INITRAMFS_CONTAINER_BUILDER:-}" \
	--env KERNEL_CONTAINER_BUILDER="${KERNEL_CONTAINER_BUILDER:-}" \
	--env OVMF_CONTAINER_BUILDER="${OVMF_CONTAINER_BUILDER:-}" \
	--env QEMU_CONTAINER_BUILDER="${QEMU_CONTAINER_BUILDER:-}" \
	--env SHIM_V2_CONTAINER_BUILDER="${SHIM_V2_CONTAINER_BUILDER:-}" \
	--env TDSHIM_CONTAINER_BUILDER="${TDSHIM_CONTAINER_BUILDER:-}" \
	--env VIRTIOFSD_CONTAINER_BUILDER="${VIRTIOFSD_CONTAINER_BUILDER:-}" \
	-v "${kata_dir}:${kata_dir}" \
	--rm \
	-w ${script_dir} \
	build-kata-deploy "${kata_deploy_create}" $@

if [ $remove_dot_docker_dir == true ]; then
	rm -rf "$HOME/.docker"
fi
