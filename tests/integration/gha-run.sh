#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

integration_dir="$(dirname "$(readlink -f "$0")")"
repo_root_dir="$(cd "${integration_dir}/../../" && pwd)"
tools_dir="${repo_root_dir}/tools"

function _print_cluster_name() {
    short_sha="$(git rev-parse --short=12 HEAD)"
    echo "${GH_PR_NUMBER}-${short_sha}-${KATA_HYPERVISOR}-${KATA_HOST_OS}-amd64"
}

function install_azure_cli() {
    curl -sL https://aka.ms/InstallAzureCLIDeb | sudo bash
    # The aks-preview extension is required while the Mariner Kata host is in preview.
    az extension add --name aks-preview
}

function login_azure() {
    az login \
        --service-principal \
        -u "${AZ_APPID}" \
        -p "${AZ_PASSWORD}" \
        --tenant "${AZ_TENANT_ID}"
}

function create_cluster() {
    # First, ensure that the cluster didn't fail to get cleaned up from a previous run.
    delete_cluster || true

    az aks create \
        -g "kataCI" \
        -n "$(_print_cluster_name)" \
        -s "Standard_D4s_v5" \
        --node-count 1 \
        --generate-ssh-keys \
        $([ "${KATA_HOST_OS}" = "cbl-mariner" ] && echo "--os-sku AzureLinux --workload-runtime KataMshvVmIsolation")
}

function install_bats() {
    sudo apt-get update
    sudo apt-get -y install bats
}

function install_kubectl() {
    sudo az aks install-cli
}

function get_cluster_credentials() {
    az aks get-credentials \
        -g "kataCI" \
        -n "$(_print_cluster_name)"
}

function ensure_yq() {
    : "${GOPATH:=${GITHUB_WORKSPACE}}"
    export GOPATH
    export PATH="${GOPATH}/bin:${PATH}"
    INSTALL_IN_GOPATH=true "${repo_root_dir}/ci/install_yq.sh"
}

function run_tests() {
    platform="${1}"
    ensure_yq

    # Emsure we're in the default namespace
    kubectl config set-context --current --namespace=default

    # Delete any spurious tests namespace that was left behind
    kubectl delete namespace kata-containers-k8s-tests &> /dev/null || true

    sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
    if [ "${KATA_HOST_OS}" = "cbl-mariner" ]; then
        yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[+].name' "HOST_OS"
        yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[-1].value' "${KATA_HOST_OS}"
    fi
    cat "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" | grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" || die "Failed to setup the tests image"

    kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
    if [ "${platform}" = "tdx" ]; then
        kubectl apply -k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/k3s"
    else
        kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
    fi
    kubectl -n kube-system wait --timeout=10m --for=condition=Ready -l name=kata-deploy pod
    kubectl apply -f "${tools_dir}/packaging/kata-deploy/runtimeclasses/kata-runtimeClasses.yaml"

    echo "Gather information about the nodes and pods after having kata-deploy ready"
    get_nodes_and_pods_info

    # This is needed as the kata-deploy pod will be set to "Ready" when it starts running,
    # which may cause issues like not having the node properly labeled or the artefacts
    # properly deployed when the tests actually start running.
    if [ "${platform}" = "aks" ]; then
        sleep 240s
    else
        sleep 60s
    fi

    # Create a new namespace for the tests and switch to it
    kubectl apply -f ${integration_dir}/kubernetes/runtimeclass_workloads/tests-namespace.yaml
    kubectl config set-context --current --namespace=kata-containers-k8s-tests

    echo "Gather information about the nodes and pods just before starting the tests"
    get_nodes_and_pods_info

    pushd "${integration_dir}/kubernetes"
    bash setup.sh
    bash run_kubernetes_tests.sh
    popd
}

function cleanup() {
    platform="${1}"

    echo "Gather information about the nodes and pods before cleaning up the node"
    get_nodes_and_pods_info "yes"

    # Switch back to the default namespace and delete the tests one
    kubectl config set-context --current --namespace=default
    kubectl delete namespace kata-containers-k8s-tests

    if [ "${platform}" = "tdx" ]; then
        deploy_spec="-k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/k3s""
        cleanup_spec="-k "${tools_dir}/packaging/kata-deploy/kata-cleanup/overlays/k3s""
    else
        deploy_spec="-f "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml""
        cleanup_spec="-f "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml""
    fi

    kubectl delete ${deploy_spec}
    kubectl -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod

    sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" | grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" || die "Failed to setup the tests image"
    kubectl apply ${cleanup_spec}
    sleep 180s

    kubectl delete ${cleanup_spec}
    kubectl delete -f "${tools_dir}/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
    kubectl delete -f "${tools_dir}/packaging/kata-deploy/runtimeclasses/kata-runtimeClasses.yaml"
}

function delete_cluster() {
    az aks delete \
        -g "kataCI" \
        -n "$(_print_cluster_name)" \
        --yes
}

function get_nodes_and_pods_info() {
    describe_pods="${1:-"no"}"

    kubectl get nodes -o wide --show-labels=true
    kubectl get pods -A
    if [[ "${describe_pods}" == "yes" ]]; then
    	kubectl describe pods -A
    fi
}

function main() {
    export KATA_HOST_OS="${KATA_HOST_OS:-}"

    action="${1:-}"

    case "${action}" in
        install-azure-cli) install_azure_cli ;;
        login-azure) login_azure ;;
        create-cluster) create_cluster ;;
        install-bats) install_bats ;;
        install-kubectl) install_kubectl ;;
        get-cluster-credentials) get_cluster_credentials ;;
        run-tests-aks) run_tests "aks" ;;
        run-tests-sev) run_tests "sev" ;;
        run-tests-snp) run_tests "snp" ;;
        run-tests-tdx) run_tests "tdx" ;;
        cleanup-sev) cleanup "sev" ;;
        cleanup-snp) cleanup "snp" ;;
        cleanup-tdx) cleanup "tdx" ;;
        delete-cluster) delete_cluster ;;
        *) >&2 echo "Invalid argument"; exit 2 ;;
    esac
}

main "$@"
