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
#where packaing repo lives
packaging_repo_dir=$(cd "${script_dir}/.." && pwd)
export USE_DOCKER=1
http_proxy=${http_proxy:-}
https_proxy=${https_proxy:-}
no_proxy=${no_proxy:-}
PUSH=${PUSH:-}
BUILD_HEAD="${BUILD_HEAD:-false}"

# shellcheck source=scripts/obs-docker.sh
source "${script_dir}/scripts/obs-docker.sh"

GO_ARCH=$(go env GOARCH)
export GO_ARCH

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

get_image() {
	pushd "${script_dir}/kata-containers-image/"
	local branch="${1:-}"
	if [ -z "${branch}" ]; then
		echo "branch not provided"
		return 1
	fi
	if [ ${BUILD_HEAD} = "false" ] && "${script_dir}/download_image.sh" "${branch}"; then
		echo "OK image downloaded"
		find . -name 'kata-containers-'"${branch}"'-*.tar.gz' || die "Failed to find downloaded image"
		return 0
	fi
	echo "Building image"
	"${script_dir}/../obs-packaging/kata-containers-image/build_image.sh" -v "${branch}"
	find . -name 'kata-containers-'"${branch}"'-*.tar.gz' || die "built image not found"
	popd
}

main() {
	local branch="${1:-}"
	[ -n "${branch}" ] || usage "missing branch" "1"
	#Build all kata packages
	make -f "${script_dir}/Makefile" clean
	get_image "${branch}"
	docker_run "${packaging_repo_dir}/obs-packaging/build_all.sh ${branch}"
}

main "$@"
