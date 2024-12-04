#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

kubernetes_dir=$(dirname "$(readlink -f "$0")")
source "${kubernetes_dir}/../../common.bash"

cleanup() {
	# Clean up all node debugger pods whose name starts with `custom-node-debugger` if pods exist
	pods_to_be_deleted=$(kubectl get pods -n kube-system --no-headers -o custom-columns=:metadata.name \
		| grep '^custom-node-debugger' || true)
	[ -n "$pods_to_be_deleted" ] && kubectl delete pod -n kube-system $pods_to_be_deleted || true
}

trap cleanup EXIT

TARGET_ARCH="${TARGET_ARCH:-x86_64}"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
K8S_TEST_DEBUG="${K8S_TEST_DEBUG:-false}"
K8S_TEST_HOST_TYPE="${K8S_TEST_HOST_TYPE:-small}"
# Setting to "yes" enables fail fast, stopping execution at the first failed test.
K8S_TEST_FAIL_FAST="${K8S_TEST_FAIL_FAST:-no}"

if [ -n "${K8S_TEST_UNION:-}" ]; then
	K8S_TEST_UNION=($K8S_TEST_UNION)
else
	# Before we use containerd 2.0 with 'image pull per runtime class' feature
	# we need run k8s-guest-pull-image.bats test first, otherwise the test result will be affected
	# by other cases which are using 'alpine' and 'quay.io/prometheus/busybox:latest' image.
	# more details https://github.com/kata-containers/kata-containers/issues/8337
	K8S_TEST_SMALL_HOST_ATTESTATION_REQUIRED_UNION=( \
		"k8s-guest-pull-image-encrypted.bats" \
		"k8s-guest-pull-image-authenticated.bats" \
		"k8s-guest-pull-image-signature.bats" \
		"k8s-confidential-attestation.bats" \
	)

	K8S_TEST_SMALL_HOST_UNION=( \
		"k8s-guest-pull-image.bats" \
		"k8s-confidential.bats" \
		"k8s-sealed-secret.bats" \
		"k8s-attach-handlers.bats" \
		"k8s-block-volume.bats" \
		"k8s-caps.bats" \
		"k8s-configmap.bats" \
		"k8s-copy-file.bats" \
		"k8s-cpu-ns.bats" \
		"k8s-credentials-secrets.bats" \
		"k8s-cron-job.bats" \
		"k8s-custom-dns.bats" \
		"k8s-empty-dirs.bats" \
		"k8s-env.bats" \
		"k8s-exec.bats" \
		"k8s-file-volume.bats" \
		"k8s-hostname.bats" \
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
		"k8s-policy-hard-coded.bats" \
		"k8s-policy-deployment.bats" \
		"k8s-policy-job.bats" \
		"k8s-policy-pod.bats" \
		"k8s-policy-pvc.bats" \
		"k8s-policy-rc.bats" \
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
			K8S_TEST_UNION=(${K8S_TEST_SMALL_HOST_ATTESTATION_REQUIRED_UNION[@]} ${K8S_TEST_SMALL_HOST_UNION[@]})
			;;
		normal)
			K8S_TEST_UNION=(${K8S_TEST_NORMAL_HOST_UNION[@]})
			;;
		all|baremetal)
			K8S_TEST_UNION=(${K8S_TEST_SMALL_HOST_ATTESTATION_REQUIRED_UNION[@]} ${K8S_TEST_SMALL_HOST_UNION[@]} ${K8S_TEST_NORMAL_HOST_UNION[@]})
			;;
		baremetal-attestation)
			K8S_TEST_UNION=(${K8S_TEST_SMALL_HOST_ATTESTATION_REQUIRED_UNION[@]})
			;;
		baremetal-no-attestation)
			K8S_TEST_UNION=(${K8S_TEST_SMALL_HOST_UNION[@]} ${K8S_TEST_NORMAL_HOST_UNION[@]})
			;;
		*)
			echo "${K8S_TEST_HOST_TYPE} is an invalid K8S_TEST_HOST_TYPE option. Valid options are: small | normal | all | baremetal"
			return 1
			;;
	esac
fi

# we may need to skip a few test cases when running on non-x86_64 arch
arch_config_file="${kubernetes_dir}/filter_out_per_arch/${TARGET_ARCH}.yaml"
if [ -f "${arch_config_file}" ]; then
	arch_k8s_test_union=$(${kubernetes_dir}/filter_k8s_test.sh ${arch_config_file} "${K8S_TEST_UNION[*]}")
	mapfile -d " " -t K8S_TEST_UNION <<< "${arch_k8s_test_union}"
fi

ensure_yq

info "Running tests with bats version: $(bats --version)"

tests_fail=()
for K8S_TEST_ENTRY in ${K8S_TEST_UNION[@]}
do
	info "$(kubectl get pods --all-namespaces 2>&1)"
	info "Executing ${K8S_TEST_ENTRY}"
	if ! bats --show-output-of-passing-tests "${K8S_TEST_ENTRY}"; then
		tests_fail+=("${K8S_TEST_ENTRY}")
		[ "${K8S_TEST_FAIL_FAST}" = "yes" ] && break
	fi
done

[ ${#tests_fail[@]} -ne 0 ] && die "Tests FAILED from suites: ${tests_fail[*]}"

info "All tests SUCCEEDED"
