#!/usr/bin/env bats
#
# Copyright (c) 2024 Edgeless Systems GmbH
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."
	setup_common || die "setup_common failed"
	pod_name="policy-pod-pvc"
	pvc_name="policy-dev"
	volume_name="policy-dev-block-pv"
	vol_capacity="10Mi"

	correct_pod_yaml="${pod_config_dir}/k8s-policy-pod-pvc.yaml"
	incorrect_pod_yaml="${pod_config_dir}/k8s-policy-pod-pvc-incorrect.yaml"
	pvc_yaml="${pod_config_dir}/k8s-policy-pvc.yaml"

	# On qemu-tdx / qemu-snp the cluster has no default StorageClass, so we create local block
	# storage (loop device + StorageClass + PV) for the PVC to bind to.
	node="$(get_one_kata_node)"
	if [[ "${RUNS_ON_AKS:-false}" == "false" ]]; then
		tmp_disk_image=$(exec_host "${node}" mktemp --tmpdir disk.XXXXXX.img | tr -d '\r\n')
		exec_host "${node}" dd if=/dev/zero of="${tmp_disk_image}" bs=1M count=0 seek=10
		loop_dev=$(exec_host "${node}" sudo losetup -f | tr -d '\r\n')
		exec_host "${node}" sudo losetup "${loop_dev}" "${tmp_disk_image}"

		kubectl apply -f - <<EOF
kind: StorageClass
apiVersion: storage.k8s.io/v1
metadata:
  name: local-storage
  annotations:
    storageclass.kubernetes.io/is-default-class: "true"
provisioner: kubernetes.io/no-provisioner
volumeBindingMode: WaitForFirstConsumer
EOF
		kubectl delete pv "${volume_name}" --ignore-not-found=true || true
		kubectl wait --for=delete "pv/${volume_name}" --timeout=30s 2>/dev/null || true
		tmp_pv_yaml=$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)
		sed -e "s|LOOP_DEVICE|${loop_dev}|" \
			-e "s|HOSTNAME|${node}|g" \
			-e "s|CAPACITY|${vol_capacity}|" \
			-e "s|block-loop-pv|${volume_name}|" \
			"${BATS_TEST_DIRNAME}/volume/block-loop-pv.yaml" > "${tmp_pv_yaml}"
		kubectl create -f "${tmp_pv_yaml}"
	fi

	# Save some time by executing genpolicy a single time.
	if [ "${BATS_TEST_NUMBER}" == "1" ]; then
		# Add policy to the correct pod yaml file
		auto_generate_policy "${pod_config_dir}" "${correct_pod_yaml}"
	fi

	# Start each test case with a copy of the correct yaml files.
	cp "${correct_pod_yaml}" "${incorrect_pod_yaml}"
}

# Ensure PVC is gone so this test can create it (idempotent; avoids AlreadyExists from previous test).
delete_pvc_if_exists() {
	kubectl delete pvc "${pvc_name}" --ignore-not-found=true || true
	kubectl wait --for=delete "pvc/${pvc_name}" --timeout=60s 2>/dev/null || true
}

@test "Successful pod with auto-generated policy" {
	delete_pvc_if_exists
	kubectl create -f "${correct_pod_yaml}"
	kubectl create -f "${pvc_yaml}"

	cmd="kubectl wait --for=condition=Ready --timeout=0s pod ${pod_name}"
	abort_cmd="kubectl describe pod ${pod_name} | grep \"CreateContainerRequest is blocked by policy\""
	info "Waiting ${wait_time}s with sleep ${sleep_time}s for: ${cmd}. Abort if: ${abort_cmd}."
	waitForCmdWithAbortCmd "${wait_time}" "${sleep_time}" "${cmd}" "${abort_cmd}"
}

# Common function for several test cases from this bats script.
test_pod_policy_error() {
	delete_pvc_if_exists
	kubectl create -f "${incorrect_pod_yaml}"
	kubectl create -f "${pvc_yaml}"
	wait_for_blocked_request "CreateContainerRequest" "${pod_name}"
}

@test "Policy failure: unexpected device mount" {
	# Changing the location of a mounted device after policy generation should fail the policy check.
	yq -i \
		'.spec.containers[0].volumeDevices.[0].devicePath = "/dev/unexpected"' \
		"${incorrect_pod_yaml}" \

	test_pod_policy_error
}

teardown() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

	# Debugging information. Don't print the "Message:" line because it contains a truncated policy log.
	kubectl describe pod "${pod_name}" | grep -v "Message:"

	# Clean-up: remove pod first so the PVC can be released, then remove PVC.
	kubectl delete -f "${correct_pod_yaml}" --ignore-not-found=true || true
	kubectl delete -f "${incorrect_pod_yaml}" --ignore-not-found=true || true
	kubectl wait --for=delete "pod/${pod_name}" --timeout=60s 2>/dev/null || true
	delete_pvc_if_exists
	rm -f "${incorrect_pod_yaml}"

	# Remove local block storage on qemu-tdx / qemu-snp (we created it in setup).
	if [[ "${RUNS_ON_AKS:-false}" == "false" ]]; then
		kubectl delete pv "${volume_name}" --ignore-not-found=true || true
		kubectl delete storageclass local-storage --ignore-not-found=true || true
		rm -f "${tmp_pv_yaml:-}"
		if [ -n "${node:-}" ] && [ -n "${loop_dev:-}" ]; then
			exec_host "${node}" sudo losetup -d "${loop_dev}" 2>/dev/null || true
			exec_host "${node}" rm -f "${tmp_disk_image:-}" 2>/dev/null || true
		fi
	fi

	teardown_common "${node}" "${node_start_time:-}"
}
