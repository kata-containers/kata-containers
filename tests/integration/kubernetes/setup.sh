#!/usr/bin/env bash
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir=$(dirname "$(readlink -f "$0")")
repo_root_dir="$(cd "${kubernetes_dir}/../../../" && pwd)"

set_runtime_class() {
    sed -i -e "s|runtimeClassName: kata|runtimeClassName: kata-${KATA_HYPERVISOR}|" ${kubernetes_dir}/runtimeclass_workloads/*.yaml
}

set_kernel_path() {
    if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
        mariner_kernel_path="/usr/share/cloud-hypervisor/vmlinux.bin"
        find ${kubernetes_dir}/runtimeclass_workloads/*.yaml -exec yq write -i {} 'metadata.annotations[io.katacontainers.config.hypervisor.kernel]' "${mariner_kernel_path}" \;
    fi
}

set_initrd_path() {
    if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
        initrd_path="/opt/kata/share/kata-containers/kata-containers-initrd-mariner.img"
        find ${kubernetes_dir}/runtimeclass_workloads/*.yaml -exec yq write -i {} 'metadata.annotations[io.katacontainers.config.hypervisor.initrd]' "${initrd_path}" \;
    fi
}

main() {
    set_runtime_class
    set_kernel_path
    set_initrd_path
}

main "$@"
