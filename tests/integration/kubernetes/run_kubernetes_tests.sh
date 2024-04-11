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

if [ -n "${K8S_TEST_UNION:-}" ]; then
	K8S_TEST_UNION=($K8S_TEST_UNION)
else
	# Before we use containerd 2.0 with 'image pull per runtime class' feature
	# we need run k8s-guest-pull-image.bats test first, otherwise the test result will be affected
	# by other cases which are using 'alpine' and 'quay.io/prometheus/busybox:latest' image.
	# more details https://github.com/kata-containers/kata-containers/issues/8337
	K8S_TEST_SMALL_HOST_UNION=( \
		"k8s-guest-pull-image.bats" \
	)

	if [ "${GENPOLICY_PULL_METHOD}" == "containerd" ]; then
		K8S_TEST_SMALL_HOST_UNION+=("k8s-pod-manifest-v1.bats")
	fi

	K8S_TEST_NORMAL_HOST_UNION=( \
		
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

# we may need to skip a few test cases when running on non-x86_64 arch
arch_config_file="${kubernetes_dir}/filter_out_per_arch/${TARGET_ARCH}.yaml"
if [ -f "${arch_config_file}" ]; then
	arch_k8s_test_union=$(${kubernetes_dir}/filter_k8s_test.sh ${arch_config_file} "${K8S_TEST_UNION[*]}")
	mapfile -d " " -t K8S_TEST_UNION <<< "${arch_k8s_test_union}"
fi

ensure_yq

for K8S_TEST_ENTRY in ${K8S_TEST_UNION[@]}
do
	info "$(kubectl get pods --all-namespaces 2>&1)"
	info "Executing ${K8S_TEST_ENTRY}"
	bats --show-output-of-passing-tests "${K8S_TEST_ENTRY}"
done
