#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

_obs_docker_packaging_repo_dir=$(cd $(dirname "${BASH_SOURCE[0]}") && cd ../.. && pwd)
GO_ARCH=$(go env GOARCH)

setup_oscrc() {
	# oscrc exists at different places on different distros
	[ -f "${HOME}/.config/osc/oscrc" ] && OSCRC="${HOME}/.config/osc/oscrc"
	OSCRC=${OSCRC:-"${HOME}/.oscrc"}
	(
		# do not log OBS credentials even in debug mode
		set +x
		OBS_API="https://api.opensuse.org"

		if [ -n "${OBS_USER:-}" ] && [ -n "${OBS_PASS:-}" ] && [ ! -e "${OSCRC}" ]; then
			echo "Creating  ${OSCRC} with user $OBS_USER"
			mkdir -p $(dirname $OSCRC)
			cat <<eom >"${OSCRC}"
[general]
apiurl = ${OBS_API}
[${OBS_API}]
user = ${OBS_USER}
pass = ${OBS_PASS}
eom
		fi
	) >>/dev/null
	if [ ! -e "${OSCRC}" ]; then
		echo "${OSCRC}, please  do 'export OBS_USER=your_user ; export OBS_PASS=your_pass' to configure osc for first time."
		exit 1
	fi
	echo "OK - osc configured"
}

docker_run() {
	local cmd="$*"
	local obs_image="obs-kata"
	#where results will be stored
	local host_datadir="${PWD}/pkgs"
	local cache_dir=${PWD}/obs-cache
	setup_oscrc

	sudo docker build \
		--build-arg http_proxy="${http_proxy:-}" \
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
		--env OBS_PROJECT="${OBS_PROJECT:-}" \
		--env OBS_SUBPROJECT="${OBS_SUBPROJECT:-}" \
		-v "${cache_dir}":/var/tmp/osbuild-packagecache/ \
		-v "${_obs_docker_packaging_repo_dir}":"${_obs_docker_packaging_repo_dir}" \
		-v "${host_datadir}":/var/packaging \
		-v "${OSCRC}":/root/.oscrc \
		-v "${PWD}":"${PWD}" \
		-w "${PWD}" \
		-ti "${obs_image}" bash -c "${cmd}"
}
