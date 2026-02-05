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
	# SMB requires privileged containers which may not work on all hypervisors
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	setup_common || die "setup_common failed"

	smb_server_name="smb-server"
	smb_client_name="smb-client-test"
	smb_server_yaml="${pod_config_dir}/smb-server.yaml"
	smb_client_yaml=$(mktemp --tmpdir smb_client_config.XXXXXX.yaml)
	test_content="hello-from-smb-share"

	# Deploy SMB server (runs as regular container, not Kata)
	kubectl apply -f "${smb_server_yaml}"

	# Wait for SMB server to be ready
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${smb_server_name}"

	# Get the SMB server IP
	smb_server_ip=$(kubectl get pod "${smb_server_name}" -o jsonpath='{.status.podIP}')
	[ -n "${smb_server_ip}" ] || die "Failed to get SMB server IP"

	# Give Samba a moment to fully initialize
	sleep 5

	# Create a test file on the SMB share
	kubectl exec "${smb_server_name}" -- sh -c "echo '${test_content}' > /share/testfile.txt"

	# Set extended attribute on the server side (will be read from client)
	kubectl exec "${smb_server_name}" -- sh -c "apk add --no-cache attr && setfattr -n user.test -v servervalue /share/testfile.txt"

	# Create client pod yaml with the actual SMB server IP
	sed -e "s|SMB_SERVER_IP|${smb_server_ip}|g" "${pod_config_dir}/pod-smb-volume.yaml" > "${smb_client_yaml}"

	# Add policy to the client pod yaml
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	# Commands we'll execute in the test
	add_exec_to_policy_settings "${policy_settings_dir}" "sh" "-c"
	add_exec_to_policy_settings "${policy_settings_dir}" "cat" "/mnt/smb/testfile.txt"
	add_exec_to_policy_settings "${policy_settings_dir}" "getfattr" "-d" "/mnt/smb/testfile.txt"
	add_exec_to_policy_settings "${policy_settings_dir}" "getfattr" "-n" "user.test" "--only-values" "/mnt/smb/testfile.txt"
	add_exec_to_policy_settings "${policy_settings_dir}" "test" "-f" "/tmp/mount-ready"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${smb_client_yaml}"

	# Create the SMB client pod (runs with Kata runtime)
	kubectl apply -f "${smb_client_yaml}"

	# Wait for client pod to be running
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${smb_client_name}"

	# Wait for the SMB mount to complete inside the container
	wait_for_smb_mount "${smb_client_name}"
}

wait_for_smb_mount() {
	local pod_name=$1
	local max_wait=${timeout%s}
	local count=0

	echo "Waiting for SMB mount to be ready..."
	while [ $count -lt $max_wait ]; do
		if kubectl exec "${pod_name}" -- test -f /tmp/mount-ready 2>/dev/null; then
			echo "SMB mount is ready"
			return 0
		fi
		sleep 2
		count=$((count + 2))
	done
	echo "Timeout waiting for SMB mount"
	return 1
}

@test "Mount SMB share and read file" {
	# Verify the SMB mount worked by reading the test file
	result=$(kubectl exec "${smb_client_name}" -- cat /mnt/smb/testfile.txt)
	echo "Read from SMB share: ${result}"
	[ "${result}" == "${test_content}" ]
}

@test "SMB share supports extended attributes (xattr) with getfattr" {
	# Read back the extended attribute (set on server in setup) using getfattr from the client
	result=$(kubectl exec "${smb_client_name}" -- getfattr -n user.test --only-values /mnt/smb/testfile.txt)
	echo "xattr value read from client: ${result}"
	[ "${result}" == "servervalue" ]
}

@test "SMB share can list all extended attributes" {
	# List all extended attributes from client - should include the one set on server
	result=$(kubectl exec "${smb_client_name}" -- getfattr -d /mnt/smb/testfile.txt 2>&1)
	echo "All xattrs: ${result}"
	echo "${result}" | grep -q "user.test"
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	# Cleanup client pod
	kubectl delete pod "${smb_client_name}" --ignore-not-found=true || true
	rm -f "${smb_client_yaml}"

	# Cleanup SMB server resources
	kubectl delete -f "${smb_server_yaml}" --ignore-not-found=true || true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
