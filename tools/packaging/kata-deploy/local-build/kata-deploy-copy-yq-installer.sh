#!/usr/bin/env bash
#
# Copyright (c) 2018-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[[ -z "${DEBUG}" ]] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname "$(readlink -f "$0")")
install_yq_script_path="${script_dir}/../../../../ci/install_yq.sh"
versions_file="${script_dir}/../../../../versions.yaml"

# Parallel targets each stage the docker build context on the host. When the
# destination does not exist yet (fresh clone), coreutils cp opens it with
# O_CREAT|O_EXCL, so concurrent copies race and the losers fail with
# "File exists". Write to a private temp file and rename: rename is atomic,
# never returns EEXIST, and all writers produce identical content.
for source in "${install_yq_script_path}" "${versions_file}"; do
	name=$(basename "${source}")
	tmp="$(mktemp "${script_dir}/dockerbuild/.${name}.XXXXXX")"
	cp "${source}" "${tmp}"
	# Preserve the executable bit for the installer.
	chmod --reference="${source}" "${tmp}"
	mv -f "${tmp}" "${script_dir}/dockerbuild/${name}"
done
