#!/usr/bin/env bash
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir=$(dirname "$(readlink -f "$0")")
source "${kubernetes_dir}/../../common.bash"

reset_workloads_work_dir() {
    rm -rf ${kubernetes_dir}/runtimeclass_workloads_work
    cp -R ${kubernetes_dir}/runtimeclass_workloads ${kubernetes_dir}/runtimeclass_workloads_work
}

set_runtime_class() {
    sed -i -e "s|runtimeClassName: kata|runtimeClassName: kata-${KATA_HYPERVISOR}|" ${kubernetes_dir}/runtimeclass_workloads_work/*.yaml
}

set_kernel_path() {
    if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
        mariner_kernel_path="/usr/share/cloud-hypervisor/vmlinux.bin"
        # Not using find -exec as that still returns 0 on failure.
        find ${kubernetes_dir}/runtimeclass_workloads_work/*.yaml -print0 | xargs -0 -I% yq write -i % 'metadata.annotations[io.katacontainers.config.hypervisor.kernel]' "${mariner_kernel_path}"
    fi
}

set_initrd_path() {
    if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
        initrd_path="/opt/kata/share/kata-containers/kata-containers-initrd-mariner.img"
        find ${kubernetes_dir}/runtimeclass_workloads_work/*.yaml -print0 | xargs -0 -I% yq write -i % 'metadata.annotations[io.katacontainers.config.hypervisor.initrd]' "${initrd_path}"
    fi
}

main() {
    ensure_yq
    reset_workloads_work_dir
    set_runtime_class
    set_kernel_path
    set_initrd_path
}

main "$@"
