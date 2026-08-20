#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Host kernel module loading in "job" mode, on a real node.
#
# The template tests pin down which stage is privileged and what it mounts, but
# the stage runs the host's modprobe under a chroot, so only a real node shows
# whether it loaded the right modules.
#
# Required environment variables:
#   DOCKER_REGISTRY - Container registry for kata-deploy image
#   DOCKER_REPO     - Repository name for kata-deploy image
#   DOCKER_TAG      - Image tag to test
#   KATA_HYPERVISOR - Hypervisor to test (qemu, clh, etc.)
#   KUBERNETES      - Must be kubeadm
#   SNAPSHOTTER     - Must be erofs
#   EROFS_DMVERITY  - Must be dmverity

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

MODULES_LOAD_FILE="/host/etc/modules-load.d/kata-containers-default.conf"
KATA_POD_NAME="kata-deploy-host-modules-verify"

# The module names the install recorded on the node, space separated.
persisted_modules() {
	run_on_host "grep -v '^#' ${MODULES_LOAD_FILE} | tr '\n' ' '"
}

# Reports each module as "<name>=loaded" or "<name>=missing", checking
# /sys/module as well because a built-in module never reaches /proc/modules.
module_states() {
	local modules="${1}"
	run_on_host "for m in ${modules}; do if [ -d /host/sys/module/\$m ] || cut -d' ' -f1 /host/proc/modules | grep -qx \$m; then echo \$m=loaded; else echo \$m=missing; fi; done"
}

setup_file() {
	if [[ "${KUBERNETES}" != "kubeadm" ]]; then
		skip "host-module integration coverage is kubeadm-only"
	fi

	if [[ "${KATA_HYPERVISOR}" == "remote" ]]; then
		skip "a remote hypervisor needs no host virtualization modules"
	fi

	if [[ "${SNAPSHOTTER:-}" != "erofs" || "${EROFS_DMVERITY:-}" != "dmverity" ]]; then
		skip "host-module integration coverage requires EROFS with dm-verity"
	fi

	ensure_helm
	echo "# Deploying kata-deploy in job mode with EROFS and dm-verity..." >&3
	deploy_kata "" --set deploymentMode=job
}

@test "Job mode loads and persists the host modules the install needs" {
	run persisted_modules
	echo "# Persisted modules: ${output}" >&3
	[ "${status}" -eq 0 ]

	# This job runs QEMU, which does reach the agent through the host's VSOCK.
	[[ "${output}" == *"vhost_vsock"* ]]

	# It also runs EROFS with dm-verity, so their modules are persisted too.
	[[ "${output}" == *"erofs"* ]]
	[[ "${output}" == *"loop"* ]]
	[[ "${output}" == *"dm_mod"* ]]
	[[ "${output}" == *"dm_verity"* ]]

	local modules="${output}"
	run module_states "${modules}"
	echo "# Module states: ${output}" >&3
	[ "${status}" -eq 0 ]
	[[ "${output}" != *"=missing"* ]]
}

@test "A Kata pod starts with EROFS after the modules are loaded" {
	kubectl delete pod "${KATA_POD_NAME}" --ignore-not-found --wait=true

	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${KATA_POD_NAME}
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

	kubectl wait --for=condition=Ready "pod/${KATA_POD_NAME}" --timeout=180s

	run kubectl exec "${KATA_POD_NAME}" -- uname -r
	echo "# Kata guest kernel: ${output}" >&3
	[ "${status}" -eq 0 ]
	[[ -n "${output}" ]]

	kubectl delete pod "${KATA_POD_NAME}" --wait=true
}

@test "Uninstall forgets the modules without unloading them" {
	local modules
	run persisted_modules
	[ "${status}" -eq 0 ]
	modules="${output}"

	uninstall_kata
	kubectl wait nodes --timeout=300s --all --for condition=Ready=True

	run run_on_host "test -e ${MODULES_LOAD_FILE} && echo PRESENT || echo GONE"
	echo "# Persistence file after uninstall: ${output}" >&3
	[[ "${output}" == *"GONE"* ]]

	# Module state is host-global and may be in use by something else, so
	# uninstall leaves the modules loaded.
	run module_states "${modules}"
	echo "# Module states after uninstall: ${output}" >&3
	[[ "${output}" != *"=missing"* ]]
}

teardown_file() {
	kubectl delete pod "${KATA_POD_NAME}" --ignore-not-found --wait=false 2>/dev/null || true
	uninstall_kata 2>/dev/null || true
}
