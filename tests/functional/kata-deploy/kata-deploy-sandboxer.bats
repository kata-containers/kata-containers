#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0
# Render-only checks; no cluster required.

CHART_PATH="${BATS_TEST_DIRNAME}/../../../tools/packaging/kata-deploy/helm-chart/kata-deploy"

@test "Helm sandboxer defaults leave generic runtimes on podsandbox" {
    run helm template kata-deploy "${CHART_PATH}"
    [ "$status" -eq 0 ]
    ! [[ "$output" == *SANDBOXER_HANDLER_MAPPING* ]]
}

@test "Helm sandboxer permits explicit podsandbox override and validates typos" {
    run helm template kata-deploy "${CHART_PATH}" -f "${CHART_PATH}/try-kata-nvidia-cpu.values.yaml" --set shims.qemu-nvidia-cpu-runtime-rs.containerd.sandboxer=podsandbox
    [ "$status" -eq 0 ]
    [[ "$output" == *"qemu-nvidia-cpu-runtime-rs:podsandbox"* ]]
    run helm template kata-deploy "${CHART_PATH}" -f "${CHART_PATH}/try-kata-nvidia-cpu.values.yaml" --set shims.qemu-nvidia-cpu-runtime-rs.containerd.sandboxer=typo
    [ "$status" -ne 0 ]
    [[ "$output" == *"Invalid containerd sandboxer"* ]]
}

@test "Helm custom runtime sandboxer override reaches the architecture-filtered list" {
    cat > "${BATS_TEST_TMPDIR}/custom.yaml" <<'YAML'
customRuntimes:
  enabled: true
  runtimes:
    example:
      baseConfig: qemu-nvidia-cpu-runtime-rs
      runtimeClass: |
        apiVersion: node.k8s.io/v1
        kind: RuntimeClass
        metadata:
          name: kata-custom-example
        handler: kata-custom-example
      containerd:
        sandboxer: podsandbox
YAML
    run helm template kata-deploy "${CHART_PATH}" -f "${CHART_PATH}/try-kata-nvidia-cpu.values.yaml" -f "${BATS_TEST_TMPDIR}/custom.yaml"
    [ "$status" -eq 0 ]
    [[ "$output" == *"kata-custom-example:qemu-nvidia-cpu-runtime-rs:::amd64,arm64:podsandbox"* ]]
}

@test "Helm omits availability socket mounts without SandboxAPI requests" {
    run helm template kata-deploy "${CHART_PATH}" --set deploymentMode=job
    [ "$status" -eq 0 ]
    ! [[ "$output" == *containerd-sandboxer-runtime-dir* ]]
    ! [[ "$output" == *CONTAINERD_SANDBOXER_SOCKET* ]]
}
