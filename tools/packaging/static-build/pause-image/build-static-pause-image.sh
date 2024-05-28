#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

[ -n "$pause_image_repo" ] || die "failed to get pause-image repo"
[ -n "$pause_image_version" ] || die "failed to get pause-image version"

pull_pause_image_from_remote() {
	echo "pull pause image from remote"

	skopeo copy "${pause_image_repo}":"${pause_image_version}" oci:pause:"${pause_image_version}"
	umoci unpack --rootless --image pause:"${pause_image_version}"  "${DESTDIR}/pause_bundle"
	rm "${DESTDIR}/pause_bundle/umoci.json"
}

pull_pause_image_from_remote "$@"
