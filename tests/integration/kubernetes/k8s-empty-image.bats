#!/usr/bin/env bats
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common || die "setup_common failed"
	pod_name="no-layer-image"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/${pod_name}.yaml"

	# genpolicy fails for this unusual container image, so use the allow_all policy.
	add_allow_all_policy_to_yaml "${yaml_file}"
}

@test "Test image with no layers cannot run" {
	# Error from run-k8s-tests (ubuntu, qemu, small):
	#
	# failed to create containerd task: failed to create shim task: the file sleep was not found
	#
	# Error from run-k8s-tests-on-tee (sev-snp, qemu-snp):
	#
	# failed to create containerd task: failed to create shim task: rpc status:
	# Status { code: INTERNAL, message: "[CDH] [ERROR]: Image Pull error: Failed to pull image
	# ghcr.io/kata-containers/no-layer-image:latest from all mirror/mapping locations or original location: image:
	# ghcr.io/kata-containers/no-layer-image:latest, error: Internal error", details: [], special_fields:
	# SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }
	#
	# Error from run-k8s-tests-coco-nontee-with-erofs-snapshotter (qemu-coco-dev, erofs, default):
	#
	# failed to create containerd task: failed to create shim task: failed to mount
	# /run/kata-containers/shared/containers/fadd1af7ea2a7bfc6caf26471f70e9a913a2989fd4a1be9d001b59e48c0781aa/rootfs
	# to /run/kata-containers/fadd1af7ea2a7bfc6caf26471f70e9a913a2989fd4a1be9d001b59e48c0781aa/rootfs, with error:
	# ENOENT: No such file or directory

	kubectl create -f "${yaml_file}"

	local -r command="kubectl describe "pod/${pod_name}" | grep -E \
		'the file sleep was not found|\[CDH\] \[ERROR\]: Image Pull error|ENOENT: No such file or directory'"
	info "Waiting ${wait_time} seconds for: ${command}"
	waitForProcess "${wait_time}" "${sleep_time}" "${command}" >/dev/null 2>/dev/null
}

teardown() {
	# Debugging information
	kubectl describe "pod/${pod_name}"
	kubectl get "pod/${pod_name}" -o yaml

	kubectl delete pod "${pod_name}"

	teardown_common "${node}" "${node_start_time:-}"
}
