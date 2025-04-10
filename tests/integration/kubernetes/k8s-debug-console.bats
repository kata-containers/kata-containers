#!/usr/bin/env bats
# Copyright (c) 2025 Advanced Micro Devices, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/lib.sh"

# For kata-runtime
export KATA_HOME="/opt/kata"
export KATA_RUNTIME="${KATA_HOME}/bin/kata-runtime"

timeout=${timeout:-30}

setup() {
  if [[ "${K8S_TEST_HOST_TYPE}" != "baremetal" ]]; then
    skip "Debug console test only implemented for baremetal systems"
  fi

  pod_name="busybox-base-pod"
  get_pod_config_dir
  yaml_file="${pod_config_dir}/pod-busybox-base.yaml"
}

@test "Access and verify that the debug console is working" {
  # Create pod
  k8s_create_pod "${yaml_file}"

  # Check pod creation
  kubectl wait --for=condition=Ready --timeout=${timeout} pod "${pod_name}"

  # Get sandbox ID
  sandbox_id=$(get_kata_sandbox_id_by_pod_name "${pod_name}")
  echo "Sandbox ID for pod [${pod_name}]: ${sandbox_id}"

  # Test debug console
  local kata_agent_path=$(sudo "${KATA_RUNTIME}" exec "${sandbox_id}" which kata-agent)
  if [[ ! "${kata_agent_path}" =~ "kata-agent" ]]; then
    echo "ERROR: The debug console could not locate the kata-agent: ${kata_agent_path}" >&2
    return 1
  fi
}

teardown() {
  if [[ "${K8S_TEST_HOST_TYPE}" != "baremetal" ]]; then
    skip "Debug console test only implemented for baremetal systems"
  fi

  # Debugging information
  kubectl describe "pod/${pod_name}"

  kubectl delete pod "${pod_name}"
}
