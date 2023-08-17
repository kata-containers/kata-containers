#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kata_deploy_dir="$(dirname "$(readlink -f "$0")")"
source "$kata_deploy_dir}/../../gha-run-k8s-common.sh"
tools_dir="${repo_root_dir}/tools"

function run_tests() {
	return 0
}

function main() {
    export KATA_HOST_OS="${KATA_HOST_OS:-}"

    action="${1:-}"

    case "${action}" in
        install-azure-cli) install_azure_cli ;;
        login-azure) login_azure ;;
        create-cluster) create_cluster "kata-deploy" ;;
        install-bats) install_bats ;;
        install-kubectl) install_kubectl ;;
        get-cluster-credentials) get_cluster_credentials "kata-deploy" ;;
        run-tests) run_tests ;;
        delete-cluster) cleanup "aks" "kata-deploy" ;;
        *) >&2 echo "Invalid argument"; exit 2 ;;
    esac
}

main "$@"
