#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir="$(dirname "$(readlink -f "$0")")"
source "${kubernetes_dir}/../../common.bash"
tools_dir="${repo_root_dir}/tools"

AZ_RG="${AZ_RG:-kataCI}"

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
        -g "${AZ_RG}" \
        -n "$(_print_cluster_name)" \
        -s "Standard_D4s_v5" \
        --node-count 1 \
        --generate-ssh-keys \
        $([ "${KATA_HOST_OS}" = "cbl-mariner" ] && echo "--os-sku AzureLinux --workload-runtime KataMshvVmIsolation")
}

function install_bats() {
    # Installing bats from the lunar repo.
    # This installs newer version of the bats which supports setup_file and teardown_file functions.
    # These functions are helpful when adding new tests that require one time setup.

    sudo apt install -y software-properties-common
    sudo add-apt-repository 'deb http://archive.ubuntu.com/ubuntu/ lunar universe'
    sudo apt install -y bats
    sudo add-apt-repository --remove 'deb http://archive.ubuntu.com/ubuntu/ lunar universe'
}

function install_kubectl() {
    sudo az aks install-cli
}

function get_cluster_credentials() {
    az aks get-credentials \
        -g "${AZ_RG}" \
        -n "$(_print_cluster_name)"
}

function deploy_kata() {
    platform="${1}"
    ensure_yq

    # Emsure we're in the default namespace
    kubectl config set-context --current --namespace=default

    sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"

    # Enable debug for Kata Containers
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[1].value' --tag '!!str' "true"
    # Create the runtime class only for the shim that's being tested
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[2].value' "${KATA_HYPERVISOR}"
    # Set the tested hypervisor as the default `kata` shim
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[3].value' "${KATA_HYPERVISOR}"
    # Let the `kata-deploy` script take care of the runtime class creation / removal
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[4].value' --tag '!!str' "true"
    # Let the `kata-deploy` create the default `kata` runtime class
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[5].value' --tag '!!str' "true"

    if [ "${KATA_HOST_OS}" = "cbl-mariner" ]; then
        yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[+].name' "HOST_OS"
        yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[-1].value' "${KATA_HOST_OS}"
    fi

    echo "::group::Final kata-deploy.yaml that is used in the test"
    cat "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" | grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" || die "Failed to setup the tests image"
    echo "::endgroup::"

    kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
    if [ "${platform}" = "tdx" ]; then
        kubectl apply -k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/k3s"
    else
        kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
    fi
    kubectl -n kube-system wait --timeout=10m --for=condition=Ready -l name=kata-deploy pod

    # This is needed as the kata-deploy pod will be set to "Ready" when it starts running,
    # which may cause issues like not having the node properly labeled or the artefacts
    # properly deployed when the tests actually start running.
    if [ "${platform}" = "aks" ]; then
        sleep 240s
    else
        sleep 60s
    fi

    echo "::group::kata-deploy logs"
    kubectl -n kube-system logs -l name=kata-deploy
    echo "::endgroup::"

    echo "::group::Runtime classes"
    kubectl get runtimeclass
    echo "::endgroup::"
}

function run_tests() {
    # Delete any spurious tests namespace that was left behind
    kubectl delete namespace kata-containers-k8s-tests &> /dev/null || true

    # Create a new namespace for the tests and switch to it
    kubectl apply -f ${kubernetes_dir}/runtimeclass_workloads/tests-namespace.yaml
    kubectl config set-context --current --namespace=kata-containers-k8s-tests

    pushd "${kubernetes_dir}"
    bash setup.sh
    bash run_kubernetes_tests.sh
    popd
}

function cleanup() {
    platform="${1}"
    ensure_yq

    echo "Gather information about the nodes and pods before cleaning up the node"
    get_nodes_and_pods_info

    if [ "${platform}" = "aks" ]; then
        delete_cluster
        return
    fi

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

    # Let the `kata-deploy` script take care of the runtime class creation / removal
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" 'spec.template.spec.containers[0].env[4].value' --tag '!!str' "true"
    # Create the runtime class only for the shim that's being tested
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" 'spec.template.spec.containers[0].env[2].value' "${KATA_HYPERVISOR}"
    # Set the tested hypervisor as the default `kata` shim
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" 'spec.template.spec.containers[0].env[3].value' "${KATA_HYPERVISOR}"
    # Let the `kata-deploy` create the default `kata` runtime class
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[5].value' --tag '!!str' "true"

    sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" | grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" || die "Failed to setup the tests image"
    kubectl apply ${cleanup_spec}
    sleep 180s

    kubectl delete ${cleanup_spec}
    kubectl delete -f "${tools_dir}/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
}

function delete_cluster() {
    az aks delete \
        -g "${AZ_RG}" \
        -n "$(_print_cluster_name)" \
        --yes
}

function get_nodes_and_pods_info() {
    kubectl debug $(kubectl get nodes -o name) -it --image=quay.io/kata-containers/kata-debug:latest || true
    kubectl get pods -o name | grep node-debugger | xargs kubectl delete || true
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
        deploy-kata-aks) deploy_kata "aks" ;;
        deploy-kata-sev) deploy_kata "sev" ;;
        deploy-kata-snp) deploy_kata "snp" ;;
        deploy-kata-tdx) deploy_kata "tdx" ;;
        run-tests) run_tests ;;
        cleanup-sev) cleanup "sev" ;;
        cleanup-snp) cleanup "snp" ;;
        cleanup-tdx) cleanup "tdx" ;;
        delete-cluster) cleanup "aks" ;;
        *) >&2 echo "Invalid argument"; exit 2 ;;
    esac
}

main "$@"
