#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

_obs_docker_packaging_repo_dir=$(cd $(dirname "${BASH_SOURCE[0]}") && cd ../.. && pwd)
GO_ARCH=$(go env GOARCH)

docker_run() {
	local cmd="$*"
	local obs_image="obs-kata"
	#where results will be stored
	local host_datadir="${PWD}/pkgs"
	local cache_dir=${PWD}/obs-cache
	sudo docker build \
		--quiet \
		--build-arg http_proxy="${http_proxy:-}" \
		--build-arg GO_ARCH="${GO_ARCH}" \
		--build-arg https_proxy="${https_proxy:-}" \
		-t $obs_image "${_obs_docker_packaging_repo_dir}/obs-packaging"

	sudo docker run \
		--rm \
		--env http_proxy="${http_proxy:-}" \
		--env https_proxy="${https_proxy:-}" \
		--env no_proxy="${no_proxy:-}" \
		--env GO_ARCH="${GO_ARCH}" \
		--env PUSH="${PUSH:-}" \
		--env DEBUG="${DEBUG:-}" \
		--env OBS_SUBPROJECT="${OBS_SUBPROJECT:-}" \
		-v "${cache_dir}":/var/tmp/osbuild-packagecache/ \
		-v "${_obs_docker_packaging_repo_dir}":"${_obs_docker_packaging_repo_dir}" \
		-v "${host_datadir}":/var/packaging \
		-v "${HOME}/.oscrc":/root/.oscrc \
		-v "${PWD}":"${PWD}" \
		-w "${PWD}" \
		-ti "${obs_image}" bash -c "${cmd}"
}
