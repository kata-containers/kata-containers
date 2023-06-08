#!/usr/bin/env bash
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir=$(dirname "$(readlink -f "$0")")

set_runtime_class() {
    sed -i -e "s|runtimeClassName: kata|runtimeClassName: kata-${KATA_HYPERVISOR}|" ${kubernetes_dir}/runtimeclass_workloads/*.yaml
}

main() {
    set_runtime_class
}

main "$@"
