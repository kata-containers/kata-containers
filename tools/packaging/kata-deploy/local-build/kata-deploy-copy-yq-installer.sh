#!/usr/bin/env bash
#
# Copyright (c) 2018-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname "$(readlink -f "$0")")
install_yq_script_path="${script_dir}/../../../../ci/install_yq.sh"

cp "${install_yq_script_path}" "${script_dir}/dockerbuild/install_yq.sh"
