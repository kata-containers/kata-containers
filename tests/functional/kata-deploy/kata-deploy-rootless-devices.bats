#!/usr/bin/env bats
#
# Copyright (c) 2026 Kata Containers community
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

UDEV_RULE="/host/etc/udev/rules.d/99-kata-containers-rootless-default.rules"

device_ownership() {
	run_on_host "test -e /host/dev/${1} && stat -c '%U %G %a' /host/dev/${1} || echo missing"
}

host_group_gid() {
	run_on_host "getent group ${1} 2>/dev/null | cut -d: -f3 | grep . || echo missing"
}

setup_file() {
	if [[ "${KUBERNETES}" != "kubeadm" ]]; then
		skip "rootless device provisioning coverage is kubeadm-only"
	fi

	case "${KATA_HYPERVISOR}" in
	qemu*-runtime-rs) ;;
	*) skip "rootless VMM is supported by the runtime-rs QEMU shims only" ;;
	esac

	ensure_helm
	echo "# Deploying kata-deploy in job mode with rootless enabled..." >&3
	deploy_kata "" --set deploymentMode=job --set rootless=true
}

@test "Rootless provisioning gives an unprivileged VMM access to /dev/kvm" {
	run device_ownership kvm
	echo "# /dev/kvm: ${output}" >&3
	[ "${status}" -eq 0 ]
	[[ "${output}" != *missing* ]]

	local group mode
	group=$(echo "${output}" | awk '{print $2}')
	mode=$(echo "${output}" | awk '{print $3}')

	[[ "${group}" != "root" ]]
	[[ "${mode:${#mode}-2:1}" =~ ^[67]$ ]]
}

@test "Provisioning leaves /dev/vhost-vsock out of it" {
	run run_on_host "grep -c vhost-vsock ${UDEV_RULE} 2>/dev/null || echo 0"
	echo "# vhost-vsock mentions in the udev rule: ${output}" >&3
	[[ "$(echo "${output}" | tr -d '[:space:]')" == "0" ]]
}

@test "The group a provisioned device names exists in the node's database" {
	local group
	run device_ownership kvm
	[ "${status}" -eq 0 ]
	group=$(echo "${output}" | awk '{print $2}')

	run host_group_gid "${group}"
	echo "# group ${group}: ${output}" >&3
	[ "${status}" -eq 0 ]
	[[ "${output}" != *missing* ]]

	[[ "$(echo "${output}" | tr -d '[:space:]')" != "0" ]]
}

@test "Provisioning is recorded where it survives a reboot" {
	run run_on_host "cat ${UDEV_RULE} 2>/dev/null || echo MISSING"
	echo "# udev rule: ${output}" >&3
	[ "${status}" -eq 0 ]
	[[ "${output}" != *MISSING* ]]

	[[ "${output}" == *'KERNEL=="kvm"'* ]]
	[[ "${output}" == *'MODE="0660"'* ]]
}

@test "A Kata pod starts with a rootless VMM" {
	local pod_name="kata-deploy-rootless-verify"
	kubectl delete pod "${pod_name}" --ignore-not-found --wait=true

	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${pod_name}
spec:
  runtimeClassName: kata-${KATA_HYPERVISOR}
  restartPolicy: Never
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  containers:
    - name: test
      image: quay.io/kata-containers/alpine-bash-curl:latest
      imagePullPolicy: Always
      command: ["sleep", "300"]
EOF

	kubectl wait --for=condition=Ready "pod/${pod_name}" --timeout=180s

	run run_on_host "ps -o user= -C qemu-system-x86_64 2>/dev/null | sort -u || true"
	echo "# QEMU users on the node: ${output}" >&3
	[[ "${output}" != *root* ]]

	kubectl delete pod "${pod_name}" --wait=true
}

@test "Uninstall forgets the device provisioning without reverting it" {
	uninstall_kata
	kubectl wait nodes --timeout=300s --all --for condition=Ready=True

	run run_on_host "test -e ${UDEV_RULE} && echo PRESENT || echo GONE"
	echo "# udev rule after uninstall: ${output}" >&3
	[[ "${output}" == *GONE* ]]

	run device_ownership kvm
	echo "# /dev/kvm after uninstall: ${output}" >&3
	[[ "${output}" != *missing* ]]
}

teardown_file() {
	kubectl delete pod kata-deploy-rootless-verify --ignore-not-found --wait=false 2>/dev/null || true
	uninstall_kata 2>/dev/null || true
}
