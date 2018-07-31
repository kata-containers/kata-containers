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
${script_name} <kata-branch/tag>
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
	#Build all kata packages
	docker_run "${packaging_repo_dir}/obs-packaging/build_all.sh ${branch}"
}

main "$@"
