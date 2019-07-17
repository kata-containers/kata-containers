#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

readonly script_dir=$(dirname $(readlink -f "$0"))

# Source to trap error line number
# shellcheck source=../lib/common.bash
source "${script_dir}/../lib/common.bash"
# shellcheck source=./lib.sh
source "${script_dir}/lib.sh"

option="sandbox_cgroup_only"
bk_suffix="${option}-bk"
bk_file="${KATA_ETC_CONFIG_PATH}-${bk_suffix}"

info "Check if exist ${bk_file}"

if [ -f "${bk_file}" ]; then
	info "restore backup ${KATA_ETC_CONFIG_PATH} "
	sudo mv "${bk_file}" "${KATA_ETC_CONFIG_PATH}"
else
	info "No backup to restore from ${KATA_ETC_CONFIG_PATH} "
fi
