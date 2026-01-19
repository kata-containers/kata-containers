#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

kata_deploy_dir=$(dirname "$(readlink -f "$0")")
source "${kata_deploy_dir}/../../common.bash"

# Setting to "yes" enables fail fast, stopping execution at the first failed test.
export BATS_TEST_FAIL_FAST="${BATS_TEST_FAIL_FAST:-no}"

if [[ -n "${KATA_DEPLOY_TEST_UNION:-}" ]]; then
	KATA_DEPLOY_TEST_UNION=("${KATA_DEPLOY_TEST_UNION}")
else
	KATA_DEPLOY_TEST_UNION=( \
		"kata-deploy.bats" \
	)
fi

run_bats_tests "${kata_deploy_dir}" KATA_DEPLOY_TEST_UNION
