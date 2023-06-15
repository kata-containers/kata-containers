#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

metrics_dir="$(dirname "$(readlink -f "$0")")"
source "${metrics_dir}/lib/common.bash"

function init_env() {
    metrics_onetime_init
    disable_ksm
}

function run_test_launchtimes() {
    hypervisor="${1}"

    echo "Running launchtimes tests: "
    init_env

    if [ "${hypervisor}" = 'qemu' ]; then
        echo "qemu"
        bash time/launch_times.sh -i public.ecr.aws/ubuntu/ubuntu:latest -n 20
    elif [ "${hypervisor}" = 'clh' ]; then
        echo "clh"
    fi
}

function main() {
    action="${1:-}"
    case "${action}" in
        run-test-launchtimes-qemu) run_test_launchtimes "qemu" ;;
        run-test-launchtimes-clh) run_test_launchtimes "clh" ;;
        *) >&2 echo "Invalid argument"; exit 2 ;;
    esac
}

main "$@"
