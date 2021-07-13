#!/bin/bash
#
# Copyright (c) 2018-2020 Intel Corporation
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

TTY_OPT="-i"
if [ -t 1 ]; then
	TTY_OPT="-i"
fi

if [ "$?" -eq 0 ] && [ "${script_dir}" != "${PWD}" ]; then
	ln -sf "${script_dir}/build" "${PWD}/build"
fi

docker build -q -t build-kata-deploy \
	--build-arg IMG_USER="${USER}" \
	--build-arg UID=${uid} \
	--build-arg GID=${gid} \
	"${script_dir}/dockerbuild/"
docker run ${TTY_OPT} \
	-v /var/run/docker.sock:/var/run/docker.sock \
	--user ${uid}:${gid} \
	--env USER=${USER} -v "${kata_dir}:${kata_dir}" \
	--rm \
	-w ${script_dir} \
	build-kata-deploy "${kata_deploy_create}" $@

