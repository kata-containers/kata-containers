#!/usr/bin/env bats
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Verify that mixed Kata hypervisors can use the shared immutable rootfs image
# concurrently, independent of sandbox startup order.

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

MIXED_POD_LABEL="kata-deploy-mixed-shims"

run_on_host() {
	local cmd="$1"
	local node_name
	node_name=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')
	local pod_name="mixed-host-exec-${RANDOM}"
	local phase=""

	kubectl run "${pod_name}" \
		--image=quay.io/kata-containers/alpine-bash-curl:latest \
		--restart=Never \
		--overrides="{
			\"spec\": {
				\"nodeName\": \"${node_name}\",
				\"hostPID\": true,
				\"activeDeadlineSeconds\": 300,
				\"tolerations\": [{\"operator\": \"Exists\"}],
				\"containers\": [{
					\"name\": \"exec\",
					\"image\": \"quay.io/kata-containers/alpine-bash-curl:latest\",
					\"imagePullPolicy\": \"IfNotPresent\",
					\"command\": [\"sh\", \"-c\", \"${cmd}\"],
					\"securityContext\": {\"privileged\": true}
				}]
			}
		}" > /dev/null

	local deadline=$((SECONDS + 60))
	while (( SECONDS < deadline )); do
		phase=$(kubectl get pod "${pod_name}" -o jsonpath='{.status.phase}' 2>/dev/null) || true
		case "${phase}" in
			Succeeded|Failed) break ;;
		esac
		sleep 1
	done

	kubectl logs "${pod_name}" 2>/dev/null || true
	kubectl delete pod "${pod_name}" --ignore-not-found=true > /dev/null 2>&1
	[[ "${phase}" == "Succeeded" ]]
}

cleanup_mixed_pods() {
	kubectl delete pods -l "test=${MIXED_POD_LABEL}" \
		--ignore-not-found=true --wait=true --timeout=180s
}

assert_kata_processes_stopped() {
	run run_on_host "kata_processes() { found=1; for proc in /proc/[0-9]*; do exe=\$(readlink \${proc}/exe 2>/dev/null || true); case \${exe} in *qemu-system*|*cloud-hypervisor*|*containerd-shim-kata-v2*) echo \${proc##*/} \${exe}; found=0;; esac; state=\$(awk '{print \$3}' \${proc}/stat 2>/dev/null || true); comm=\$(awk '{print \$2}' \${proc}/stat 2>/dev/null || true); if [ x\${state} = xZ ]; then case \${comm} in *qemu-system*|*cloud-hypervis*|*containerd-shim*) echo \${proc##*/} zombie \${comm}; found=0;; esac; fi; done; return \${found}; }; for attempt in \$(seq 1 30); do if ! kata_processes >/dev/null; then exit 0; fi; sleep 1; done; kata_processes; exit 1"
	if [[ "${status}" -ne 0 ]]; then
		echo "${output}" >&3
	fi
	[[ "${status}" -eq 0 ]]
}

create_mixed_pod() {
	local scenario="$1"
	local shim="$2"
	local node_name="$3"
	local pod_name="mixed-${scenario}-${shim}"

	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${pod_name}
  labels:
    test: ${MIXED_POD_LABEL}
spec:
  nodeName: ${node_name}
  runtimeClassName: kata-${shim}
  restartPolicy: Never
  containers:
    - name: hold
      image: quay.io/kata-containers/alpine-bash-curl:latest
      imagePullPolicy: IfNotPresent
      command: ["sleep", "infinity"]
EOF

	if ! kubectl wait --for=condition=Ready "pod/${pod_name}" --timeout=180s; then
		kubectl describe pod "${pod_name}" >&3 || true
		kubectl get events --sort-by=.lastTimestamp >&3 || true
		return 1
	fi
}

exercise_startup_order() {
	local scenario="$1"
	shift
	local node_name
	node_name=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')
	local shim

	for shim in "$@"; do
		create_mixed_pod "${scenario}" "${shim}" "${node_name}"
	done

	for shim in "$@"; do
		kubectl exec "mixed-${scenario}-${shim}" -- true
	done

	cleanup_mixed_pods
	assert_kata_processes_stopped
}

setup_file() {
	ensure_helm

	local mixed_values
	mixed_values=$(mktemp)
	cat > "${mixed_values}" <<EOF
shims:
  disableAll: true
  qemu:
    enabled: true
  qemu-runtime-rs:
    enabled: true
  clh:
    enabled: true
defaultShim:
  amd64: qemu
EOF

	deploy_kata "${mixed_values}"
	rm -f "${mixed_values}"
}

@test "Mixed shims share the Kata rootfs in either startup order" {
	exercise_startup_order forward clh qemu qemu-runtime-rs
	exercise_startup_order reverse qemu qemu-runtime-rs clh
}

teardown_file() {
	cleanup_mixed_pods || true
	uninstall_kata
}
