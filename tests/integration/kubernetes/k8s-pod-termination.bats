#!/usr/bin/env bats
#
# Copyright (c) 2026 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# Regression test for kata pod stuck-in-Terminating.
#
# A multi-process workload causes the ttrpc connection to the kata-agent to
# close during StopContainer.  If the shim exits before StopPodSandbox is
# called, CRI engines without a ttrpc.ErrClosed guard on kill() will loop
# forever retrying the dead shim.
#
# The test creates a pod that spawns multiple background processes and uses
# "kill 0" on SIGTERM. After deletion it asserts the pod disappears within
# the termination grace period, proving the CRI engine handled the dead-shim
# case cleanly.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="pod-termination-multiprocess"
	setup_common || die "setup_common failed"
	yaml_file="${pod_config_dir}/${pod_name}.yaml"

	cat > "${yaml_file}" <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: ${pod_name}
spec:
  runtimeClassName: kata
  containers:
  - name: workload
    image: quay.io/prometheus/busybox:latest
    command:
    - sh
    - -c
    - |
      for i in 1 2 3 4 5; do
        sh -c 'while true; do sleep 1; done' &
      done
      trap 'sleep 1; kill 0; wait; exit 0' TERM
      wait
EOF
}

# Regression test: multi-process pod must terminate cleanly without getting
# stuck in Terminating.  Fails if kubectl delete does not complete within 60s.
@test "Multi-process pod terminates cleanly (no stuck-in-Terminating regression)" {
	# Create and wait for Running
	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	# Allow the workload to fully initialise its child processes before we
	# send SIGTERM, so the multi-process case is exercised.
	sleep 3

	# Delete and wait for the pod object to disappear.  60s gives kubelet
	# enough time to complete the full graceful-stop sequence (default 30s
	# grace period) before we declare failure.
	kubectl delete pod "${pod_name}" --wait=true --timeout=60s
}

teardown() {
	kubectl describe "pod/${pod_name}" || true
	kubectl delete pod "${pod_name}" --ignore-not-found=true --wait=false
	rm -f "${yaml_file}"
	teardown_common "${node}" "${node_start_time:-}"
}
