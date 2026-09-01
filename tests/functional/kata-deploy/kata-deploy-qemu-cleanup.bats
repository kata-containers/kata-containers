#!/usr/bin/env bats
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Verify that QEMU, Cloud Hypervisor, and Kata shim processes exit after the
# sandbox stops, instead of remaining as live processes or zombies.
#
# Required environment variables:
#   DOCKER_REGISTRY - Container registry for kata-deploy image
#   DOCKER_REPO     - Repository name for kata-deploy image
#   DOCKER_TAG      - Image tag to test
#   KATA_HYPERVISOR - Hypervisor to test (qemu, qemu-runtime-rs, clh)
#   KUBERNETES      - K8s distribution (microk8s, k3s, rke2, etc.)

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CLEANUP_POD_NAME="kata-qemu-cleanup-test"

# Inspect the host PID namespace for leftover VMMs, shims, and matching zombies.
assert_kata_processes_stopped() {
	run run_on_host "kata_processes() { found=1; for proc in /proc/[0-9]*; do exe=\$(readlink \${proc}/exe 2>/dev/null || true); case \${exe} in *qemu-system*|*cloud-hypervisor*|*containerd-shim-kata-v2*) echo \${proc##*/} \${exe}; found=0;; esac; state=\$(awk '{print \$3}' \${proc}/stat 2>/dev/null || true); comm=\$(awk '{print \$2}' \${proc}/stat 2>/dev/null || true); if [ x\${state} = xZ ]; then case \${comm} in *qemu-system*|*cloud-hypervis*|*containerd-shim*) echo \${proc##*/} zombie \${comm}; found=0;; esac; fi; done; return \${found}; }; for attempt in \$(seq 1 30); do if ! kata_processes >/dev/null; then exit 0; fi; sleep 1; done; kata_processes; exit 1" true
	if [[ "${status}" -ne 0 ]]; then
		echo "${output}" >&3
	fi
	[[ "${status}" -eq 0 ]]
}

setup_file() {
	ensure_helm
	deploy_kata
}

@test "VMM and shim processes stop after the sandbox is deleted" {
	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${CLEANUP_POD_NAME}
spec:
  runtimeClassName: kata-${KATA_HYPERVISOR}
  restartPolicy: Never
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  containers:
    - name: hold
      image: quay.io/kata-containers/alpine-bash-curl:latest
      imagePullPolicy: IfNotPresent
      command: ["sleep", "infinity"]
EOF

	if ! kubectl wait --for=condition=Ready "pod/${CLEANUP_POD_NAME}" --timeout=180s; then
		kubectl describe pod "${CLEANUP_POD_NAME}" >&3 || true
		kubectl get events --sort-by=.lastTimestamp >&3 || true
		return 1
	fi

	kubectl delete pod "${CLEANUP_POD_NAME}" --wait=true --timeout=180s
	assert_kata_processes_stopped
}

teardown_file() {
	kubectl delete pod "${CLEANUP_POD_NAME}" --ignore-not-found=true --wait=false 2>/dev/null || true
	uninstall_kata 2>/dev/null || true
}
