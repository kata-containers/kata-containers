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

# The GID rather than the group name: the pod reading the device resolves names
# against its own image, where the node's groups do not exist.
device_ownership() {
	run_on_host "test -e /host/dev/${1} && stat -c %U:%g:%a /host/dev/${1} || echo missing"
}

host_group_name() {
	run_on_host "awk -F: -v gid=${1} '\$3 == gid { print \$1 }' /host/etc/group | grep . || echo missing"
}

setup_file() {
	if [[ "${KUBERNETES}" != "kubeadm" ]]; then
		skip "rootless device provisioning coverage is kubeadm-only"
	fi

	case "${KATA_HYPERVISOR}" in
	qemu*-runtime-rs | clh*-runtime-rs) ;;
	*) skip "rootless VMM is supported by the runtime-rs QEMU and Cloud Hypervisor shims only" ;;
	esac

	ensure_helm
	echo "# Deploying kata-deploy in job mode with rootless enabled..." >&3
	deploy_kata "" --set deploymentMode=job \
		--set "shims.${KATA_HYPERVISOR}.hypervisor.rootless=true"
}

@test "Rootless provisioning gives an unprivileged VMM access to /dev/kvm" {
	run device_ownership kvm
	echo "# /dev/kvm: ${output}" >&3
	[ "${status}" -eq 0 ]
	[[ "${output}" != *missing* ]]

	local gid mode
	gid=$(echo "${output}" | cut -d: -f2)
	mode=$(echo "${output}" | cut -d: -f3)

	# Group 0 would hand the VMM user every other root-group resource instead.
	[[ "${gid}" != "0" ]]
	[[ "${mode:${#mode}-2:1}" =~ ^[67]$ ]]
}

@test "Provisioning leaves the fd-passed devices out of it" {
	run run_on_host "cat ${UDEV_RULE} 2>/dev/null || echo MISSING"
	echo "# udev rule: ${output}" >&3
	[[ "${output}" != *MISSING* ]]

	[[ "${output}" != *vhost-vsock* ]]
	[[ "${output}" != *vhost-net* ]]
}

@test "The group a provisioned device names exists in the node's database" {
	local gid
	run device_ownership kvm
	[ "${status}" -eq 0 ]
	gid=$(echo "${output}" | cut -d: -f2)

	run host_group_name "${gid}"
	echo "# group ${gid}: ${output}" >&3
	[ "${status}" -eq 0 ]
	[[ "${output}" != *missing* ]]
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

	# comm, not cmdline: this very command names the VMMs it looks for, and runs
	# as root. UIDs, not names: the per-sandbox user exists on the node, not in
	# this image.
	run run_on_host \
		"for p in /proc/[0-9]*; do grep -qs -e qemu-system -e cloud-hyperv \$p/comm && stat -c %u \$p; done | sort -u" \
		true true
	echo "# VMM process owners on the node: ${output}" >&3
	[[ -n "${output}" ]]

	local uid
	while read -r uid; do
		[[ "${uid}" != "0" ]]
	done <<< "${output}"

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
