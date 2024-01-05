#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

kubernetes_dir=$(dirname "$(readlink -f "$0")")
source "${kubernetes_dir}/../../common.bash"

TARGET_ARCH="${TARGET_ARCH:-x86_64}"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
K8S_TEST_DEBUG="${K8S_TEST_DEBUG:-false}"
K8S_TEST_HOST_TYPE="${K8S_TEST_HOST_TYPE:-small}"

ALLOW_ALL_POLICY="${ALLOW_ALL_POLICY:-$(base64 -w 0 runtimeclass_workloads_work/allow-all.rego)}"

if [ -n "${K8S_TEST_UNION:-}" ]; then
	K8S_TEST_UNION=($K8S_TEST_UNION)
else
	K8S_TEST_SMALL_HOST_UNION=( \
		"k8s-confidential.bats" \
		"k8s-attach-handlers.bats" \
		"k8s-caps.bats" \
		"k8s-configmap.bats" \
		"k8s-copy-file.bats" \
		"k8s-cpu-ns.bats" \
		"k8s-credentials-secrets.bats" \
		"k8s-custom-dns.bats" \
		"k8s-empty-dirs.bats" \
		"k8s-env.bats" \
		"k8s-exec.bats" \
		"k8s-file-volume.bats" \
		"k8s-inotify.bats" \
		"k8s-job.bats" \
		"k8s-kill-all-process-in-container.bats" \
		"k8s-limit-range.bats" \
		"k8s-liveness-probes.bats" \
		"k8s-measured-rootfs.bats" \
		"k8s-memory.bats" \
		"k8s-nested-configmap-secret.bats" \
		"k8s-oom.bats" \
		"k8s-optional-empty-configmap.bats" \
		"k8s-optional-empty-secret.bats" \
		"k8s-pid-ns.bats" \
		"k8s-pod-quota.bats" \
		"k8s-port-forward.bats" \
		"k8s-projected-volume.bats" \
		"k8s-qos-pods.bats" \
		"k8s-replication.bats" \
		"k8s-seccomp.bats" \
		"k8s-sysctls.bats" \
		"k8s-security-context.bats" \
		"k8s-shared-volume.bats" \
		"k8s-volume.bats" \
		"k8s-nginx-connectivity.bats" \
	)

	K8S_TEST_NORMAL_HOST_UNION=( \
		"k8s-number-cpus.bats" \
		"k8s-parallel.bats" \
		"k8s-sandbox-vcpus-allocation.bats" \
		"k8s-scale-nginx.bats" \
	)

	case ${K8S_TEST_HOST_TYPE} in
		small)
			K8S_TEST_UNION=(${K8S_TEST_SMALL_HOST_UNION[@]})
			;;
		normal)
			K8S_TEST_UNION=(${K8S_TEST_NORMAL_HOST_UNION[@]})
			;;
		baremetal)
			K8S_TEST_UNION=(${K8S_TEST_SMALL_HOST_UNION[@]} ${K8S_TEST_NORMAL_HOST_UNION[@]})

			;;
		*)
			echo "${K8S_TEST_HOST_TYPE} is an invalid K8S_TEST_HOST_TYPE option. Valid options are: small | normal | baremetal"
			return 1
			;;
	esac
fi

policy_tests_enabled() {
	# The Guest images for these platforms have been built using AGENT_POLICY=yes -
	# see kata-deploy-binaries.sh.
	[ "${KATA_HYPERVISOR}" == "qemu-sev" ] || [ "${KATA_HYPERVISOR}" == "qemu-snp" ] || \
		[ "${KATA_HYPERVISOR}" == "qemu-tdx" ] || [ "${KATA_HOST_OS}" == "cbl-mariner" ]
}

add_policy_to_yaml() {
	local yaml_file="$1"
	local resource_kind="$(yq read ${yaml_file} kind)"

	case "${resource_kind}" in

	Pod)
		echo "Adding policy to ${resource_kind} from ${yaml_file}"
		ALLOW_ALL_POLICY="${ALLOW_ALL_POLICY}" yq write -i "${K8S_TEST_YAML}" \
			'metadata.annotations."io.katacontainers.config.agent.policy"' \
			"${ALLOW_ALL_POLICY}"
		;;

	Deployment|Job|ReplicationController)
		echo "Adding policy to ${resource_kind} from ${yaml_file}"
		ALLOW_ALL_POLICY="${ALLOW_ALL_POLICY}" yq write -i "${K8S_TEST_YAML}" \
			'spec.template.metadata.annotations."io.katacontainers.config.agent.policy"' \
			"${ALLOW_ALL_POLICY}"
		;;

	List)
		echo "Issue #7765: adding policy to ${resource_kind} from ${yaml_file} is not implemented yet"
		;;

	ConfigMap|LimitRange|Namespace|PersistentVolume|PersistentVolumeClaim|RuntimeClass|Secret|Service)
		echo "Policy is not required for ${resource_kind} from ${yaml_file}"
		;;

	*)
		echo "k8s resource type ${resource_kind} from ${yaml_file} is not yet supported for policy testing"
		return 1
		;;

	esac
}

add_policy_to_successful_tests() {
	info "Add policy to test YAML files"
	for K8S_TEST_YAML in runtimeclass_workloads_work/*.yaml
	do
		add_policy_to_yaml "${K8S_TEST_YAML}"
	done
}

test_successful_actions() {
	info "Test actions that must be successful"
	for K8S_TEST_ENTRY in ${K8S_TEST_UNION[@]}
	do
		info "$(kubectl get pods --all-namespaces 2>&1)"
		bats "${K8S_TEST_ENTRY}"
	done
}

run_policy_specific_tests() {
	info "$(kubectl get pods --all-namespaces 2>&1)"
	bats k8s-exec-rejected.bats
	info "$(kubectl get pods --all-namespaces 2>&1)"
	bats k8s-policy-set-keys.bats
}

# we may need to skip a few test cases when running on non-x86_64 arch
arch_config_file="${kubernetes_dir}/filter_out_per_arch/${TARGET_ARCH}.yaml"
if [ -f "${arch_config_file}" ]; then
	arch_k8s_test_union=$(${kubernetes_dir}/filter_k8s_test.sh ${arch_config_file} "${K8S_TEST_UNION[*]}")
	mapfile -d " " -t K8S_TEST_UNION <<< "${arch_k8s_test_union}"
fi

if policy_tests_enabled; then
	ensure_yq
	run_policy_specific_tests
	add_policy_to_successful_tests
else
	info "Policy tests are disabled on this platform"
fi

test_successful_actions
