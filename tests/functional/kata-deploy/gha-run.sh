#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kata_deploy_dir="$(dirname "$(readlink -f "$0")")"
source "${kata_deploy_dir}/../../gha-run-k8s-common.sh"

function run_tests() {
	cleanup_runtimeclasses || true

	pushd "${kata_deploy_dir}"
	bash run-kata-deploy-tests.sh
	popd
}

function cleanup_runtimeclasses() {
	# Cleanup any runtime class that was left behind in the cluster, in
	# case of a test failure, apart from the default one that comes from
	# AKS
	for rc in `kubectl get runtimeclass -o name | grep -v "kata-mshv-vm-isolation" | sed 's|runtimeclass.node.k8s.io/||'`
	do
		kubectl delete runtimeclass $rc;
	done
}

function cleanup() {
	platform="${1}"
	test_type="${2:-k8s}"

	cleanup_runtimeclasses || true

	if [ "${platform}" = "aks" ]; then
		delete_cluster ${test_type}
	fi
}

function main() {
    export KATA_HOST_OS="${KATA_HOST_OS:-}"

    platform="aks"
    if [ "${KATA_HYPERVISOR}" = "qemu-tdx" ]; then
	    platform="tdx"
    fi
    export platform

    action="${1:-}"

    case "${action}" in
        install-azure-cli) install_azure_cli ;;
        login-azure) login_azure ;;
        create-cluster) create_cluster "kata-deploy" ;;
        deploy-k8s) deploy_k8s ;;
        install-bats) install_bats ;;
        install-kubectl) install_kubectl ;;
        get-cluster-credentials) get_cluster_credentials "kata-deploy" ;;
        run-tests) run_tests ;;
        delete-cluster) cleanup "aks" "kata-deploy" ;;
        *) >&2 echo "Invalid argument"; exit 2 ;;
    esac
}

main "$@"
