#!/bin/bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

# The kata shim to be used
export KATA_RUNTIME=${KATA_RUNTIME:-kata-qemu}

script_dir=$(dirname $0)
source ${script_dir}/lib.sh

suite=$1
if [ -z "$1" ]; then
	suite='smoke'
fi

# Make oc and kubectl visible
export PATH=/tmp/shared:$PATH

oc version || die "Test cluster is unreachable"

info "Install and configure kata into the test cluster"
export SELINUX_PERMISSIVE="no"
${script_dir}/cluster/install_kata.sh || die "Failed to install kata-containers"

info "Run test suite: $suite"
test_status='PASS'
${script_dir}/run_${suite}_test.sh || test_status='FAIL'
info "Test suite: $suite: $test_status"
[ "$test_status" == "PASS" ]
