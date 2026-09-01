#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

kata_deploy_dir=$(dirname "$(readlink -f "$0")")
# shellcheck source=/dev/null
source "${kata_deploy_dir}/../../common.bash"

# Setting to "yes" enables fail fast, stopping execution at the first failed test.
export BATS_TEST_FAIL_FAST="${BATS_TEST_FAIL_FAST:-no}"

if [[ -n "${KATA_DEPLOY_TEST_UNION:-}" ]]; then
	KATA_DEPLOY_TEST_UNION=("${KATA_DEPLOY_TEST_UNION}")
else
	KATA_DEPLOY_TEST_UNION=()

	# Only the kubeadm setup prepares EROFS, and this has to run before any
	# other suite loads the modules it is meant to exercise.
	if [[ "${KUBERNETES:-}" == "kubeadm" ]]; then
		KATA_DEPLOY_TEST_UNION+=("kata-deploy-host-modules.bats")
	fi

	KATA_DEPLOY_TEST_UNION+=( \
		"kata-deploy.bats" \
		"kata-deploy-qemu-cleanup.bats" \
		"kata-deploy-custom-runtimes.bats" \
		"kata-deploy-lifecycle.bats" \
		"kata-deploy-scheduling.bats" \
		"kata-deploy-tee-keys.bats" \
		"kata-deploy-distribution.bats" \
		"kata-deploy-privileges.bats" \
		"kata-deploy-multi-install.bats" \
		"kata-deploy-reconcile.bats" \
		"kata-deploy-node-binaries.bats" \
	)
fi

run_bats_tests "${kata_deploy_dir}" KATA_DEPLOY_TEST_UNION
