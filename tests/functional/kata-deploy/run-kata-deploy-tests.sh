#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

kata_deploy_dir=$(dirname "$(readlink -f "$0")")
source "${kata_deploy_dir}/../../common.bash"

if [ -n "${KATA_DEPLOY_TEST_UNION:-}" ]; then
	KATA_DEPLOY_TEST_UNION=($KATA_DEPLOY_TEST_UNION)
else
	KATA_DEPLOY_TEST_UNION=( \
		"kata-deploy.bats" \
	)
fi

info "Run tests"
for KATA_DEPLOY_TEST_ENTRY in ${KATA_DEPLOY_TEST_UNION[@]}
do
	bats --show-output-of-passing-tests "${KATA_DEPLOY_TEST_ENTRY}"
done
