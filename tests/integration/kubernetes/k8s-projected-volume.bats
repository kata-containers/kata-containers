#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	get_pod_config_dir
	yaml_file="${pod_config_dir}/pod-projected-volume.yaml"
}

@test "Projected volume" {
	password="1f2d1e2e67df"
	username="admin"
	pod_name="test-projected-volume"

	# TODO: disabled due to #8894
	# auto_generate_policy "${yaml_file}"

	TMP_FILE=$(mktemp username.XXXX)
	SECOND_TMP_FILE=$(mktemp password.XXXX)

	# Create files containing the username and password
	echo "$username" > $TMP_FILE
	echo "$password" > $SECOND_TMP_FILE

	# Package these files into secrets
	kubectl create secret generic user --from-file=$TMP_FILE
	kubectl create secret generic pass --from-file=$SECOND_TMP_FILE

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check that the projected sources exists
	cmd="ls /projected-volume | grep username"
	kubectl exec $pod_name -- sh -c "$cmd"
	sec_cmd="ls /projected-volume | grep password"
	kubectl exec $pod_name -- sh -c "$sec_cmd"

	# Check content of the projected sources
	check_cmd="cat /projected-volume/username*"
	kubectl exec $pod_name -- sh -c "$check_cmd" | grep "$username"
	sec_check_cmd="cat /projected-volume/password*"
	kubectl exec $pod_name -- sh -c "$sec_check_cmd" | grep "$password"
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	# Debugging information
	kubectl describe "pod/$pod_name"

	rm -f $TMP_FILE $SECOND_TMP_FILE
	kubectl delete pod "$pod_name"
	kubectl delete secret pass user
}
