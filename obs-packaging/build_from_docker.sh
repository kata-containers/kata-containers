#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
[ -z "${DEBUG}" ] || set -o xtrace

set -o errexit
set -o nounset
set -o pipefail

script_dir=$(cd $(dirname "${BASH_SOURCE[0]}") && pwd)
script_name="$(basename "${BASH_SOURCE[0]}")"
cache_dir=${PWD}/obs-cache
#where packaing repo lives
packaging_repo_dir=$(cd "${script_dir}/.." && pwd)
#where results will be stored
host_datadir="${PWD}/pkgs"
obs_image="obs-kata"
export USE_DOCKER=1
http_proxy=${http_proxy:-}
https_proxy=${https_proxy:-}
no_proxy=${no_proxy:-}
PUSH=${PUSH:-}

GO_ARCH=$(go env GOARCH)
export GO_ARCH

docker_run() {
	local cmd="$@"
	sudo docker run \
		--rm \
		-v "${HOME}/.ssh":/root/.ssh \
		-v "${HOME}/.gitconfig":/root/.gitconfig \
		-v /etc/profile:/etc/profile \
		--env GO_ARCH="${GO_ARCH}" \
		--env http_proxy="${http_proxy}" \
		--env https_proxy="${https_proxy}" \
		--env no_proxy="${no_proxy}" \
		--env PUSH="${PUSH}" \
		--env DEBUG="${DEBUG:-}" \
		--env OBS_SUBPROJECT="${OBS_SUBPROJECT:-}" \
		-v "${HOME}/.bashrc":/root/.bashrc \
		-v "$cache_dir":/var/tmp/osbuild-packagecache/ \
		-v "$packaging_repo_dir":${packaging_repo_dir} \
		-v "$host_datadir":/var/packaging \
		-v "$HOME/.oscrc":/root/.oscrc \
		-ti "$obs_image" bash -c "${cmd}"
}
usage() {
	msg="${1:-}"
	exit_code=$"${2:-0}"
	cat <<EOT
${msg}
Usage:
${script_name} <kata-branch>
EOT
	exit "${exit_code}"
}

main() {
	local branch="${1:-}"
	[ -n "${branch}" ] || usage "missing branch" "1"
	pushd "${script_dir}/kata-containers-image/" >>/dev/null
	echo "Building image"
	image_tarball=$(find . -name 'kata-containers-'"${branch}"'-*.tar.gz')
	[ -f "${image_tarball}" ] || "${script_dir}/../obs-packaging/kata-containers-image/build_image.sh" -v "${branch}"
	image_tarball=$(find . -name 'kata-containers-'"${branch}"'-*.tar.gz')
	[ -f "${image_tarball}" ] || die "image not found"
	popd >>/dev/null
	sudo docker build \
		--build-arg http_proxy="${http_proxy}" \
		--build-arg https_proxy="${https_proxy}" \
		--build-arg GO_ARCH="${GO_ARCH}" \
		-t $obs_image "${script_dir}"

	#Create/update OBS repository for branch
	#docker_run "${packaging_repo_dir}/obs-packaging/create-pkg-branch.sh ${branch}"
	#Build all kata packages
	docker_run "${packaging_repo_dir}/obs-packaging/build_all.sh ${branch}"
}

main $@
