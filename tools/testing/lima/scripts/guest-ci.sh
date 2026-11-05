#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run as the SSH user: Kata's CI scripts use sudo where required.
set -euo pipefail

LAB_CI_DIR="${HOME}/just-kata"
CI_SOURCE="${JUST_KATA_SOURCE_DIR:-${LAB_CI_DIR}/src}"
CLUSTER_MARKER="${LAB_CI_DIR}/cluster-distribution"
PREPARED_MARKER="${LAB_CI_DIR}/prepared"
export KUBECONFIG="${KUBECONFIG:-${HOME}/.kube/config}"
export GOPATH="${HOME}/go"
export PATH="${GOPATH}/bin:/usr/local/bin:/usr/local/sbin:${PATH}"
export TARGET_ARCH
: "${ARCH:?ARCH must be supplied by the lab controller}"
case "${ARCH}" in
    amd64) TARGET_ARCH=x86_64 ;;
    arm64) TARGET_ARCH=aarch64 ;;
esac
export DEBUG="${DEBUG:-}"
export SMOKE_IMAGE KUBERNETES

require_source() {
    test -f "${CI_SOURCE}/tests/integration/kubernetes/gha-run.sh" || {
        echo 'Sync a Kata Containers checkout before running CI commands' >&2; exit 1;
    }
    CI_SOURCE=$(realpath "${CI_SOURCE}")
    cd "${CI_SOURCE}"
}

load_ci_helpers() {
    require_source
    # shellcheck disable=SC1091
    source tests/gha-run-k8s-common.sh
}

enable_helm_overrides() {
    test -n "${JUST_KATA_HELM_VALUES:-}" || return 0
    # Use the upstream tool bootstrap before exporting a Helm wrapper: otherwise
    # `command -v helm` would see the function and skip installing the real CLI.
    load_ci_helpers
    ensure_helm
    mkdir -p "${LAB_CI_DIR}"
    export JUST_KATA_HELM_VALUES_FILE="${LAB_CI_DIR}/helm-overrides.yaml"
    printf '%s\n' "${JUST_KATA_HELM_VALUES}" > "${JUST_KATA_HELM_VALUES_FILE}"
    helm() {
        case "${1:-}" in
            template|install|upgrade) command helm "$@" --values "${JUST_KATA_HELM_VALUES_FILE}" ;;
            *) command helm "$@" ;;
        esac
    }
    export -f helm
}

prepare_host() {
    require_source
    # shellcheck disable=SC1091
    . /etc/os-release
    test "${ID}:${VERSION_ID}" = ubuntu:26.04 || {
        echo 'Automatic prerequisite setup currently requires Ubuntu 26.04' >&2; exit 1;
    }
    sudo -n true
    if ! test -f "${PREPARED_MARKER}"; then
        if sudo test -e /etc/kubernetes/admin.conf || sudo test -d /var/lib/rancher/k3s ||
           sudo test -d /var/lib/rancher/rke2 || sudo test -d /var/lib/k0s; then
            echo 'Existing unmanaged Kubernetes installation; attach using explicit ci-script commands instead' >&2
            exit 1
        fi
        if sudo test -e /opt/kata; then
            echo 'Existing unmanaged Kata installation; refusing automatic setup' >&2; exit 1
        fi
    fi
    sudo env DEBIAN_FRONTEND=noninteractive apt-get update
    sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates curl git gnupg lsb-release jq python3 kmod sudo \
        apt-transport-https software-properties-common build-essential
    if test "${KUBERNETES}" = microk8s; then
        sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y snapd
    fi
    # GitHub runners already have these CI prerequisites; fresh cloud images do not.
    INSTALL_IN_GOPATH=true bash ci/install_yq.sh
    mkdir -p "${LAB_CI_DIR}"
    touch "${PREPARED_MARKER}"
    echo 'Host prerequisites prepared; Kata modules and artifacts are managed by kata-deploy'
}

require_prepared() {
    if test "${EXECUTION_BACKEND:-lima}" != baremetal; then
        test -f "${PREPARED_MARKER}" || { echo 'Run prepare-host first' >&2; exit 1; }
    fi
    if test -f "${CLUSTER_MARKER}"; then
        test "$(cat "${CLUSTER_MARKER}")" = "${KUBERNETES}" || {
            echo 'Kubernetes distribution differs from the installed cluster; use a fresh VM target' >&2; exit 1;
        }
    fi
}

deploy_cluster() {
    require_prepared
    require_source
    if ! test -f "${CLUSTER_MARKER}"; then
        printf '%s\n' "${KUBERNETES}" > "${CLUSTER_MARKER}"
        bash tests/integration/kubernetes/gha-run.sh deploy-k8s
    elif ! kubectl get nodes --request-timeout=15s >/dev/null; then
        if [[ "${KUBERNETES}" = kubeadm || "${KUBERNETES}" = vanilla ]] &&
           ! sudo test -e /etc/kubernetes/admin.conf && ! sudo test -d /etc/kubernetes/manifests; then
            # Safe to repeat an attempt that failed before kubeadm initialized.
            bash tests/integration/kubernetes/gha-run.sh deploy-k8s
        else
            echo 'Previous cluster setup is incomplete; inspect the CI log before retrying deploy-k8s explicitly' >&2
            return 1
        fi
    fi
    kubectl wait --for=condition=Ready nodes --all --timeout=300s
}

deploy_kata() {
    : "${KATA_HYPERVISOR:?KATA_HYPERVISOR must be supplied by the CI configuration}"
    require_prepared
    require_source
    if kubectl get runtimeclass "kata-${KATA_HYPERVISOR}" >/dev/null 2>&1 &&
       test -n "$(kubectl get nodes -l katacontainers.io/kata-runtime=true -o name)"; then
        echo 'kata-deploy installation already advertised by the node; use ci deploy-kata to reapply'
        return
    fi
    enable_helm_overrides
    bash tests/integration/kubernetes/gha-run.sh deploy-kata
}

smoke() {
    : "${KATA_HYPERVISOR:?KATA_HYPERVISOR must be supplied by the CI configuration}"
    require_prepared
    kubectl get runtimeclass "kata-${KATA_HYPERVISOR}"
    kubectl create namespace just-kata --dry-run=client -o yaml | kubectl apply -f -
    CI_SMOKE_POD=$(python3 - <<'PY' | kubectl -n just-kata create -f - -o name
import json, os
print(json.dumps({
    'apiVersion': 'v1', 'kind': 'Pod',
    'metadata': {'generateName': 'kata-smoke-', 'labels': {'app.kubernetes.io/managed-by': 'just-kata'}},
    'spec': {
        'runtimeClassName': 'kata-' + os.environ['KATA_HYPERVISOR'], 'restartPolicy': 'Never',
        'containers': [{'name': 'smoke', 'image': os.environ['SMOKE_IMAGE'], 'imagePullPolicy': 'IfNotPresent',
                        'command': ['sh', '-ec', 'printf "Kata guest kernel: "; uname -r; test -r /proc/version']}],
    },
}))
PY
    )
    cleanup_pod() { kubectl -n just-kata delete "${CI_SMOKE_POD}" --ignore-not-found --wait=false >/dev/null || true; }
    trap cleanup_pod EXIT
    local phase
    for _ in $(seq 1 60); do
        phase=$(kubectl -n just-kata get "${CI_SMOKE_POD}" -o jsonpath='{.status.phase}')
        case "${phase}" in
            Succeeded)
                kubectl -n just-kata logs "${CI_SMOKE_POD}"
                cleanup_pod
                trap - EXIT
                echo 'READY: Kubernetes pod completed using the Kata RuntimeClass'
                return ;;
            Failed) break ;;
        esac
        sleep 3
    done
    kubectl -n just-kata describe "${CI_SMOKE_POD}" || true
    kubectl -n just-kata logs "${CI_SMOKE_POD}" || true
    return 1
}

logs() {
    uname -a
    lsmod
    free -m
    lsblk -b
    cat "${CLUSTER_MARKER}" || true
    git -C "${CI_SOURCE}" rev-parse HEAD || true
    git -C "${CI_SOURCE}" status --short || true
    kubectl version -o yaml || true
    kubectl get nodes -o wide || true
    kubectl get pods -A -o wide || true
    kubectl get runtimeclass -o yaml || true
    kubectl get events -A --sort-by=.metadata.creationTimestamp || true
    helm list -A -o json || true
    load_ci_helpers
    local mode=job
    if test -n "$(kubectl -n kube-system get daemonsets -l name=kata-deploy -o name 2>/dev/null)"; then
        mode=daemonset
    fi
    dump_kata_deploy_diagnostics "${mode}"
    sudo journalctl -u kubelet -u containerd -u k3s -u rke2-server -u k0scontroller --no-pager -n 1000 || true
}

case "${1:-}" in
    library) ;; # Source function definitions for isolated lifecycle checks.
    prepare-host) prepare_host ;;
    deploy-k8s) deploy_cluster ;;
    deploy-kata) deploy_kata ;;
    smoke) smoke ;;
    script)
        require_prepared
        require_source
        enable_helm_overrides
        shift
        script=$1
        shift
        # Resolve symlinks as well as '..' before running repository scripts.
        script=$(realpath "${script}")
        case "${script}" in "${CI_SOURCE}"/*) ;; *) echo 'CI script must be inside the selected checkout' >&2; exit 2 ;; esac
        case "${script}" in
            *.sh|*.bash) exec bash "${script}" "$@" ;;
            *.py) exec python3 "${script}" "$@" ;;
            *) exec "${script}" "$@" ;;
        esac
        ;;
    logs) logs ;;
    *) echo 'Unknown CI action' >&2; exit 2 ;;
esac
