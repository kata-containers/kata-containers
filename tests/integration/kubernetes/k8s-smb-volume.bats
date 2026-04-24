#!/usr/bin/env bats
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Test SMB/CIFS volume mounting and extended attributes (xattr) support
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	skip "test is currently flaking due to failed mounts"
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	setup_common || die "setup_common failed"

	smb_server_name="smb-server"
	smb_client_name="smb-client-test"
	smb_server_yaml="${pod_config_dir}/smb-server.yaml"
	smb_client_yaml=$(mktemp --tmpdir smb_client_config.XXXXXX.yaml)
	test_content="hello-from-smb-share"

	# Prepare policy settings (auto_generate_policy runs in the test after YAML is templated)
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${policy_settings_dir}" "sh" "-c"
	add_exec_to_policy_settings "${policy_settings_dir}" "cat" "/mnt/smb/testfile.txt"
	add_exec_to_policy_settings "${policy_settings_dir}" "getfattr" "-d" "/mnt/smb/testfile.txt"
	add_exec_to_policy_settings "${policy_settings_dir}" "getfattr" "-n" "user.test" "--only-values" "/mnt/smb/testfile.txt"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
}

@test "SMB volume mount with xattr support" {
	# Deploy SMB server (runs as regular container, not Kata)
	kubectl apply -f "${smb_server_yaml}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${smb_server_name}"

	# Get the SMB server IP
	smb_server_ip=$(kubectl get pod "${smb_server_name}" -o jsonpath='{.status.podIP}')
	[ -n "${smb_server_ip}" ] || die "Failed to get SMB server IP"

	# Give Samba a moment to fully initialize
	sleep 5

	# Create a test file on the SMB share and set an extended attribute
	kubectl exec "${smb_server_name}" -- sh -c "echo '${test_content}' > /share/testfile.txt"
	kubectl exec "${smb_server_name}" -- sh -c "apk add --no-cache attr && setfattr -n user.test -v servervalue /share/testfile.txt"

	# Create client pod yaml with the actual SMB server IP and generate policy
	sed -e "s|\$SMB_SERVER_IP|${smb_server_ip}|g" "${pod_config_dir}/pod-smb-volume.yaml" > "${smb_client_yaml}"
	auto_generate_policy "${policy_settings_dir}" "${smb_client_yaml}"

	# Deploy the SMB client pod (runs with Kata runtime)
	kubectl apply -f "${smb_client_yaml}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${smb_client_name}"

	# Verify the SMB mount worked by reading the test file
	result=$(kubectl exec "${smb_client_name}" -- cat /mnt/smb/testfile.txt)
	echo "Read from SMB share: ${result}"
	[ "${result}" == "${test_content}" ]

	# Read back the extended attribute set on the server
	result=$(kubectl exec "${smb_client_name}" -- getfattr -n user.test --only-values /mnt/smb/testfile.txt)
	echo "xattr value read from client: ${result}"
	[ "${result}" == "servervalue" ]

	# List all extended attributes - should include the one set on server
	result=$(kubectl exec "${smb_client_name}" -- getfattr -d /mnt/smb/testfile.txt 2>&1)
	echo "All xattrs: ${result}"
	echo "${result}" | grep -q "user.test"
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	rm -f "${smb_client_yaml}"
	kubectl delete configmap smb-server-config --ignore-not-found=true || true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
