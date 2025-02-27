#!/bin/bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

stability_dir="$(dirname "$(readlink -f "$0")")"
source "${stability_dir}/../metrics/lib/common.bash"
source "${stability_dir}/../gha-run-k8s-common.sh"
kata_tarball_dir="${2:-kata-artifacts}"

function run_tests() {
	info "Running scability test using ${KATA_HYPERVISOR} hypervisor"
	bash "${stability_dir}/kubernetes_stability.sh"

	info "Running soak stability test using ${KATA_HYPERVISOR} hypervisor"
	bash "${stability_dir}/kubernetes_soak_test.sh"

	info "Running stressng stability test using ${KATA_HYPERVISOR} hypervisor"
	bash "${stability_dir}/kubernetes_stressng.sh"
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-azure-cli) install_azure_cli ;;
		login-azure) login_azure ;;
		create-cluster) create_cluster ;;
		install-bats) install_bats ;;
		install-kata-tools) install_kata_tools ;;
		install-kubectl) install_kubectl ;;
		get-cluster-credentials) get_cluster_credentials ;;
		deploy-snapshotter) deploy_snapshotter ;;
		deploy-kata-aks) deploy_kata "aks" ;;
		deploy-coco-kbs) deploy_coco_kbs ;;
		install-kbs-client) install_kbs_client ;;
		run-tests) run_tests ;;
		delete-cluster) cleanup "aks" ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"

