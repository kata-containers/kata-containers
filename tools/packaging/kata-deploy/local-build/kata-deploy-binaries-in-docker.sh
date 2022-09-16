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
docker_gid=$(getent group docker | cut -d: -f3 || { echo >&2 "Missing docker group, docker needs to be installed" && false; })

# If docker gid is the effective group id of the user, do not pass it as
# an additional group.
if [ ${docker_gid} == ${gid} ]; then
	docker_gid=""
fi

docker build -q -t build-kata-deploy \
	--build-arg IMG_USER="${USER}" \
	--build-arg UID=${uid} \
	--build-arg GID=${gid} \
	--build-arg HOST_DOCKER_GID=${docker_gid} \
	"${script_dir}/dockerbuild/"

docker run \
	-v /var/run/docker.sock:/var/run/docker.sock \
	--env CI="${CI:-}" \
	--env USER=${USER} -v "${kata_dir}:${kata_dir}" \
	--rm \
	-w ${script_dir} \
	build-kata-deploy "${kata_deploy_create}" $@
