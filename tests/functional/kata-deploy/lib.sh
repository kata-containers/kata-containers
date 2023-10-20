#!/usr/bin/env bats
#
# Copyright (c) 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
set -o errexit
set -o nounset
set -o pipefail

# Print useful information for debugging kata-deploy fails
debug_kata_deploy() {
	echo "::group::Describe kata-deploy pod"
	kubectl -n kube-system describe pod -l name=kata-deploy || true
	echo "::endgroup::"

	echo "::group::Status of the k8s nodes"
	kubectl get nodes || true
	echo "::endgroup::"

	echo "::group::List of runtimeclasses"
	# When running on AKS "kata-mshv-vm-isolation" is added by the cloud
	# provider, so it's noise and should be filtered out here.
	kubectl get runtimeclasses -o name | grep -v "kata-mshv-vm-isolation" || true
	echo "::endgroup::"
}