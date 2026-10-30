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

@test "Helm NVIDIA profiles request SandboxAPI only for non-TEE Rust runtimes" {
    for profile in cpu gpu; do
        run helm template kata-deploy "${CHART_PATH}" -f "${CHART_PATH}/try-kata-nvidia-${profile}.values.yaml"
        [ "$status" -eq 0 ]
        [[ "$output" == *"qemu-nvidia-${profile}-runtime-rs:shim"* ]]
        ! [[ "$output" == *"qemu-nvidia-${profile}:shim"* ]]
        if [ "$profile" = gpu ]; then
            for tee in snp tdx; do
                [[ "$output" == *"qemu-nvidia-gpu-${tee}-runtime-rs:podsandbox"* ]]
                ! [[ "$output" == *"qemu-nvidia-gpu-${tee}-runtime-rs:shim"* ]]
            done
        fi
    done
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

@test "Helm custom empty sandboxer selects podsandbox while omission inherits" {
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
        sandboxer: ""
YAML
    run helm template kata-deploy "${CHART_PATH}" -f "${CHART_PATH}/try-kata-nvidia-cpu.values.yaml" -f "${BATS_TEST_TMPDIR}/custom.yaml"
    [ "$status" -eq 0 ]
    [[ "$output" == *"kata-custom-example:qemu-nvidia-cpu-runtime-rs:::amd64,arm64:podsandbox"* ]]
    sed '/containerd:/d; /sandboxer:/d' "${BATS_TEST_TMPDIR}/custom.yaml" > "${BATS_TEST_TMPDIR}/inherited.yaml"
    run helm template kata-deploy "${CHART_PATH}" -f "${CHART_PATH}/try-kata-nvidia-cpu.values.yaml" -f "${BATS_TEST_TMPDIR}/inherited.yaml"
    [ "$status" -eq 0 ]
    printf '%s\n' "$output" | grep -q 'kata-custom-example:qemu-nvidia-cpu-runtime-rs:::amd64,arm64:$'
}

@test "Helm availability mounts a missing-safe runtime directory only in the CRI stage" {
    for distro in k8s k3s rke2 k0s microk8s; do
        case "$distro" in
            k3s|rke2) directory=/run/k3s/containerd ;;
            k0s) directory=/run/k0s ;;
            microk8s) directory=/var/snap/microk8s/common/run ;;
            *) directory=/run/containerd ;;
        esac
        run helm template kata-deploy "${CHART_PATH}" -f "${CHART_PATH}/try-kata-nvidia-cpu.values.yaml" --set deploymentMode=job --set k8sDistribution="$distro"
        [ "$status" -eq 0 ]
        [[ "$output" == *"path: \"$directory\""* ]]
        [[ "$output" == *'value: "/run/kata-containerd-probe/containerd.sock"'* ]]
        [ "$(printf '%s\n' "$output" | grep -c 'mountPath: /run/kata-containerd-probe')" -eq 1 ]
        printf '%s\n' "$output" | grep -A3 'name: containerd-sandboxer-runtime-dir' | grep -q DirectoryOrCreate
    done
    run helm template kata-deploy "${CHART_PATH}" -f "${CHART_PATH}/try-kata-nvidia-cpu.values.yaml" --set deploymentMode=job --set containerd.runtimeSocket=unix:///custom/runtime/daemon.sock
    [ "$status" -eq 0 ]
    [[ "$output" == *'path: "/custom/runtime"'* ]]
    [[ "$output" == *'value: "/run/kata-containerd-probe/daemon.sock"'* ]]
}

@test "Helm omits availability socket mounts without SandboxAPI requests" {
    run helm template kata-deploy "${CHART_PATH}" --set deploymentMode=job
    [ "$status" -eq 0 ]
    ! [[ "$output" == *containerd-sandboxer-runtime-dir* ]]
    ! [[ "$output" == *CONTAINERD_SANDBOXER_SOCKET* ]]
}
