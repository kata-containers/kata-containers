#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

tests_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${tests_dir}/common.bash"

K8S_TEST_HOST_TYPE="${K8S_TEST_HOST_TYPE:-small}"

function _print_instance_type() {
    case ${K8S_TEST_HOST_TYPE} in
        small)
            echo "Standard_D2s_v5"
            ;;
        normal)
            echo "Standard_D4s_v5"
            ;;
        *)
            echo "Unknown instance type '${K8S_TEST_HOST_TYPE}'" >&2
            exit 1
    esac
}

function _print_cluster_name() {
    test_type="${1:-k8s}"

    short_sha="$(git rev-parse --short=12 HEAD)"
    echo "${test_type}-${GH_PR_NUMBER}-${short_sha}-${KATA_HYPERVISOR}-${KATA_HOST_OS}-amd64-${K8S_TEST_HOST_TYPE:0:1}"
}

function _print_rg_name() {
    test_type="${1:-k8s}"

    echo "${AZ_RG:-"kataCI-$(_print_cluster_name ${test_type})"}"
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
    test_type="${1:-k8s}"

    # First ensure it didn't fail to get cleaned up from a previous run.
    delete_cluster "${test_type}" || true

    local rg="$(_print_rg_name ${test_type})"

    az group create \
        -l eastus2 \
        -n "${rg}"

    az aks create \
        -g "${rg}" \
	--node-resource-group "node-${rg}" \
        -n "$(_print_cluster_name ${test_type})" \
        -s "$(_print_instance_type)" \
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
    test_type="${1:-k8s}"

    az aks get-credentials \
        -g "$(_print_rg_name ${test_type})" \
        -n "$(_print_cluster_name ${test_type})"
}

function delete_cluster() {
    test_type="${1:-k8s}"

    az group delete \
        -g "$(_print_rg_name ${test_type})" \
        --yes
}

function delete_cluster_kcli() {
	CLUSTER_NAME="${CLUSTER_NAME:-kata-k8s}"
	kcli delete -y kube "$CLUSTER_NAME"
}

function get_nodes_and_pods_info() {
    kubectl debug $(kubectl get nodes -o name) -it --image=quay.io/kata-containers/kata-debug:latest || true
    kubectl get pods -o name | grep node-debugger | xargs kubectl delete || true
}

function deploy_k0s() {
	curl -sSLf https://get.k0s.sh | sudo sh

	sudo k0s install controller --single ${KUBERNETES_EXTRA_PARAMS:-}

	sudo k0s start

	# This is an arbitrary value that came up from local tests
	sleep 120s

	# Download the kubectl binary into /usr/bin so we can avoid depending
	# on `k0s kubectl` command
	ARCH=$(uname -m)
	if [ "${ARCH}" = "x86_64" ]; then
		ARCH=amd64
	fi
	kubectl_version=$(sudo k0s kubectl version 2>/dev/null | grep "Client Version" | sed -e 's/Client Version: //')
	sudo curl -fL --progress-bar -o /usr/bin/kubectl https://storage.googleapis.com/kubernetes-release/release/${kubectl_version}/bin/linux/${ARCH}/kubectl
	sudo chmod +x /usr/bin/kubectl

	mkdir -p ~/.kube
	sudo cp /var/lib/k0s/pki/admin.conf ~/.kube/config
	sudo chown ${USER}:${USER} ~/.kube/config
}

function deploy_k3s() {
	curl -sfL https://get.k3s.io | sh -s - --write-kubeconfig-mode 644

	# This is an arbitrary value that came up from local tests
	sleep 120s

	# Download the kubectl binary into /usr/bin and remove /usr/local/bin/kubectl
	#
	# We need to do this to avoid hitting issues like:
	# ```sh
	# error: open /etc/rancher/k3s/k3s.yaml.lock: permission denied
	# ```
	# Which happens basically because k3s links `/usr/local/bin/kubectl`
	# to `/usr/local/bin/k3s`, and that does extra stuff that vanilla
	# `kubectl` doesn't do.
	ARCH=$(uname -m)
	if [ "${ARCH}" = "x86_64" ]; then
		ARCH=amd64
	fi
	kubectl_version=$(/usr/local/bin/k3s kubectl version --client=true 2>/dev/null | grep "Client Version" | sed -e 's/Client Version: //' -e 's/+k3s[0-9]\+//')
	sudo curl -fL --progress-bar -o /usr/bin/kubectl https://storage.googleapis.com/kubernetes-release/release/${kubectl_version}/bin/linux/${ARCH}/kubectl
	sudo chmod +x /usr/bin/kubectl
	sudo rm -rf /usr/local/bin/kubectl

	mkdir -p ~/.kube
	cp /etc/rancher/k3s/k3s.yaml ~/.kube/config
}

function create_cluster_kcli() {
	CLUSTER_NAME="${CLUSTER_NAME:-kata-k8s}"

	delete_cluster_kcli || true

	kcli create kube "${KUBE_TYPE:-generic}" \
		-P domain="kata.com" \
		-P pool="${LIBVIRT_POOL:-default}" \
		-P ctlplanes="${CLUSTER_CONTROL_NODES:-1}" \
		-P workers="${CLUSTER_WORKERS:-1}" \
		-P network="${LIBVIRT_NETWORK:-default}" \
		-P image="${CLUSTER_IMAGE:-ubuntu2004}" \
		-P sdn=flannel \
		-P nfs=false \
		-P disk_size="${CLUSTER_DISK_SIZE:-20}" \
		"${CLUSTER_NAME}"

	export KUBECONFIG="$HOME/.kcli/clusters/$CLUSTER_NAME/auth/kubeconfig"

	local cmd="kubectl get nodes | grep '.*worker.*\<Ready\>'"
	echo "Wait at least one worker be Ready"
	if ! waitForProcess "330" "30" "$cmd"; then
		echo "ERROR: worker nodes not ready."
		kubectl get nodes
		return 1
	fi

	# Ensure that system pods are running or completed.
	cmd="[ \$(kubectl get pods -A --no-headers | grep -v 'Running\|Completed' | wc -l) -eq 0 ]"
	echo "Wait system pods be running or completed"
	if ! waitForProcess "90" "30" "$cmd"; then
		echo "ERROR: not all pods are Running or Completed."
		kubectl get pods -A
		kubectl get pods -A
		return 1
	fi
}

function deploy_rke2() {
	curl -sfL https://get.rke2.io | sudo sh -

	sudo systemctl enable --now rke2-server.service

	# This is an arbitrary value that came up from local tests
	sleep 120s

	# Link the kubectl binary into /usr/bin
	sudo ln -sf /var/lib/rancher/rke2/bin/kubectl /usr/local/bin/kubectl

	mkdir -p ~/.kube
	sudo cp /etc/rancher/rke2/rke2.yaml ~/.kube/config
	sudo chown ${USER}:${USER} ~/.kube/config
}

function _get_k0s_kubernetes_version_for_crio() {
	# k0s version will look like:
	# v1.27.5+k0s.0
	#
	# The CRI-O repo for such version of Kubernetes expects something like:
	# 1.27
	k0s_version=$(curl -sSLf "https://docs.k0sproject.io/stable.txt")

	# Remove everything after the second '.'
	crio_version=${k0s_version%\.*+*}
	# Remove the 'v'
	crio_version=${crio_version#v}

	echo ${crio_version}
}

function setup_crio() {
	# Get the CRI-O version to be installed depending on the version of the
	# "k8s distro" that we are using
	case ${KUBERNETES} in
		k0s) crio_version=$(_get_k0s_kubernetes_version_for_crio) ;;
		*) >&2 echo "${KUBERNETES} flavour is not supported with CRI-O"; exit 2 ;;

	esac

	install_crio ${crio_version}
}

function deploy_k8s() {
	echo "::group::Deploying ${KUBERNETES}"

	case ${KUBERNETES} in
		k0s) deploy_k0s ;;
		k3s) deploy_k3s ;;
		rke2) deploy_rke2 ;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	echo "::endgroup::"
}
