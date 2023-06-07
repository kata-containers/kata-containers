#!/bin/bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e
set -x

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

source "${SCRIPT_PATH}/../../../.ci/lib.sh"
source "${SCRIPT_PATH}/../../lib/common.bash"
source "${SCRIPT_PATH}/../../../functional/tdx/lib/common-tdx.bash"
test_repo="${test_repo:-github.com/kata-containers/tests}"

function start_cassandra() {
        info "Start cassandra"
        pushd "${GOPATH}/src/${test_repo}/metrics/disk/cassandra_kubernetes"
        bash ./cassandra.sh
        popd
}



function main() {
        get_config_file
        setup_tdx
        install_qemu_tdx
        install_kernel_tdx
        enable_confidential_computing
        start_cassandra
        remove_tdx_tmp_dir
}

main "$@"
