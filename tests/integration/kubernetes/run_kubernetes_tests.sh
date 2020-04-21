#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

source /etc/os-release || source /usr/lib/os-release
kubernetes_dir=$(dirname "$(readlink -f "$0")")
cidir="${kubernetes_dir}/../../.ci/"
source "${cidir}/lib.sh"

arch="$(uname -m)"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

# Currently, Kubernetes tests only work on Ubuntu.
# We should delete this condition, when it works for other Distros.
# In the case of CentOS once that issue https://github.com/cri-o/cri-o/issues/3130
# is being fixed we can enable Kubernetes tests.
if [ "$ID" != "ubuntu" ]; then
	echo "Skip Kubernetes tests on $ID"
	echo "kubernetes tests on $ID aren't supported yet"
	exit 0
fi

if [ "$KATA_HYPERVISOR" == "firecracker" ]; then
	die "Kubernetes tests will not run with $KATA_HYPERVISOR"
fi

# Using trap to ensure the cleanup occurs when the script exists.
trap '${kubernetes_dir}/cleanup_env.sh' EXIT

# Docker is required to initialize kubeadm, even if we are
# using cri-o as the runtime.
systemctl is-active --quiet docker || sudo systemctl start docker

K8S_TEST_UNION=("k8s-attach-handlers.bats" \
	"k8s-configmap.bats" \
	"k8s-copy-file.bats" \
	"k8s-cpu-ns.bats" \
	"k8s-credentials-secrets.bats" \
	"k8s-custom-dns.bats" \
	"k8s-empty-dirs.bats" \
	"k8s-env.bats" \
	"k8s-expose-ip.bats" \
	"k8s-job.bats" \
	"k8s-limit-range.bats" \
	"k8s-liveness-probes.bats" \
	"k8s-memory.bats" \
	"k8s-number-cpus.bats" \
	"k8s-parallel.bats" \
	"k8s-pid-ns.bats" \
	"k8s-pod-quota.bats" \
	"k8s-port-forward.bats" \
	"k8s-projected-volume.bats" \
	"k8s-qos-pods.bats" \
	"k8s-replication.bats" \
	"k8s-scale-nginx.bats" \
	"k8s-security-context.bats" \
	"k8s-shared-volume.bats" \
	"k8s-uts+ipc-ns.bats" \
	"k8s-volume.bats" \
	"nginx.bats" \
	"k8s-hugepages.bats")

if [ "${KATA_HYPERVISOR:-}" == "cloud-hypervisor" ]; then
	blk_issue="https://github.com/kata-containers/tests/issues/2318"
	sysctl_issue="https://github.com/kata-containers/tests/issues/2324"
	info "blk ${blk_issue}"
	info "$KATA_HYPERVISOR sysctl is failing:"
	info "sysctls: ${sysctl_issue}"
else
	K8S_TEST_UNION+=("k8s-block-volume.bats")
	K8S_TEST_UNION+=("k8s-sysctls.bats")
fi
# we may need to skip a few test cases when running on non-x86_64 arch
if [ -f "${cidir}/${arch}/configuration_${arch}.yaml" ]; then
	config_file="${cidir}/${arch}/configuration_${arch}.yaml"
	arch_k8s_test_union=$(${cidir}/filter/filter_k8s_test.sh ${config_file} "${K8S_TEST_UNION[*]}")
	mapfile -d " " -t K8S_TEST_UNION <<< "${arch_k8s_test_union}"
fi

pushd "$kubernetes_dir"
./init.sh
for K8S_TEST_ENTRY in ${K8S_TEST_UNION[@]}
do
	bats "${K8S_TEST_ENTRY}"
done
popd
