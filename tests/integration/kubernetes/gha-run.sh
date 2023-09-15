#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir="$(dirname "$(readlink -f "$0")")"
source "${kubernetes_dir}/../../gha-run-k8s-common.sh"
tools_dir="${repo_root_dir}/tools"

function configure_devmapper() {
	sudo mkdir -p /var/lib/containerd/devmapper
	sudo truncate --size 10G /var/lib/containerd/devmapper/data-disk.img
	sudo truncate --size 10G /var/lib/containerd/devmapper/meta-disk.img

	cat<<EOF | sudo tee /etc/systemd/system/containerd-devmapper.service
[Unit]
Description=Setup containerd devmapper device
DefaultDependencies=no
After=systemd-udev-settle.service
Before=lvm2-activation-early.service
Wants=systemd-udev-settle.service
[Service]
Type=oneshot
RemainAfterExit=true
ExecStart=-/sbin/losetup /dev/loop20 /var/lib/containerd/devmapper/data-disk.img
ExecStart=-/sbin/losetup /dev/loop21 /var/lib/containerd/devmapper/meta-disk.img
[Install]
WantedBy=local-fs.target
EOF

	sudo systemctl daemon-reload
	sudo systemctl enable --now containerd-devmapper

	# Time to setup the thin pool for consumption.
	# The table arguments are such.
	# start block in the virtual device
	# length of the segment (block device size in bytes / Sector size (512)
	# metadata device
	# block data device
	# data_block_size Currently set it 512 (128KB)
	# low_water_mark. Copied this from containerd snapshotter test setup
	# no. of feature arguments
	# Skip zeroing blocks for new volumes.
	sudo dmsetup create contd-thin-pool \
	        --table "0 20971520 thin-pool /dev/loop21 /dev/loop20 512 32768 1 skip_block_zeroing"
	
	case "${KUBERNETES}" in
		k3s)
			containerd_config_file="/var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl"
			sudo cp /var/lib/rancher/k3s/agent/etc/containerd/config.toml ${containerd_config_file}
			;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	# We're not using this with baremetal machines, so we're fine on cutting
	# corners here and just append this to the configuration file.
	cat<<EOF | sudo tee -a ${containerd_config_file}
[plugins."io.containerd.snapshotter.v1.devmapper"]
  pool_name = "contd-thin-pool"
  base_image_size = "4096MB"
EOF

	case "${KUBERNETES}" in
		k3s)
			sudo sed -i -e 's/snapshotter = "overlayfs"/snapshotter = "devmapper"/g' ${containerd_config_file}
			sudo systemctl restart k3s ;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	sleep 60s
	sudo cat ${containerd_config_file}
}

function configure_snapshotter() {
	echo "::group::Configuring ${SNAPSHOTTER}"

	case ${SNAPSHOTTER} in
		devmapper) configure_devmapper ;;
		*) >&2 echo "${SNAPSHOTTER} flavour is not supported"; exit 2 ;;
	esac

	echo "::endgroup::"
}

function deploy_kata() {
    platform="${1}"
    ensure_yq

    # Emsure we're in the default namespace
    kubectl config set-context --current --namespace=default

    sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"

    # Enable debug for Kata Containers
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[1].value' --tag '!!str' "true"
    # Let the `kata-deploy` script take care of the runtime class creation / removal
    yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[4].value' --tag '!!str' "true"

    if [ "${KATA_HOST_OS}" = "cbl-mariner" ]; then
        yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[+].name' "HOST_OS"
        yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[-1].value' "${KATA_HOST_OS}"
    fi

    echo "::group::Final kata-deploy.yaml that is used in the test"
    cat "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" | grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" || die "Failed to setup the tests image"
    echo "::endgroup::"

    kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
    if [ "${KUBERNETES}" = "k3s" ]; then
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
    kubectl -n kube-system logs --tail=100 -l name=kata-deploy
    echo "::endgroup::"

    echo "::group::Runtime classes"
    kubectl get runtimeclass
    echo "::endgroup::"
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
	kubectl_version=$(/usr/local/bin/k3s kubectl version --short 2>/dev/null | grep "Client Version" | sed -e 's/Client Version: //' -e 's/\+k3s1//')
	sudo curl -fL --progress-bar -o /usr/bin/kubectl https://storage.googleapis.com/kubernetes-release/release/${kubectl_version}/bin/linux/${ARCH}/kubectl
	sudo chmod +x /usr/bin/kubectl
	sudo rm -rf /usr/local/bin/kubectl

	mkdir -p ~/.kube
	cp /etc/rancher/k3s/k3s.yaml ~/.kube/config
}

function deploy_k8s() {
	echo "::group::Deploying ${KUBERNETES}"

	case ${KUBERNETES} in
		k3s) deploy_k3s ;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

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
    test_type="${2:-k8s}"
    ensure_yq

    echo "Gather information about the nodes and pods before cleaning up the node"
    get_nodes_and_pods_info

    if [ "${platform}" = "aks" ]; then
        delete_cluster ${test_type}
        return
    fi

    # Switch back to the default namespace and delete the tests one
    kubectl config set-context --current --namespace=default
    kubectl delete namespace kata-containers-k8s-tests

    if [ "${KUBERNETES}" = "k3s" ]; then
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

    sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
    cat "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" | grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" || die "Failed to setup the tests image"
    kubectl apply ${cleanup_spec}
    sleep 180s

    kubectl delete ${cleanup_spec}
    kubectl delete -f "${tools_dir}/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
}

function main() {
    export KATA_HOST_OS="${KATA_HOST_OS:-}"
    export K8S_TEST_HOST_TYPE="${K8S_TEST_HOST_TYPE:-}"

    action="${1:-}"

    case "${action}" in
        install-azure-cli) install_azure_cli ;;
        login-azure) login_azure ;;
        create-cluster) create_cluster ;;
        configure-snapshotter) configure_snapshotter ;;
        deploy-k8s) deploy_k8s ;;
        install-bats) install_bats ;;
        install-kubectl) install_kubectl ;;
        get-cluster-credentials) get_cluster_credentials ;;
        deploy-kata-aks) deploy_kata "aks" ;;
        deploy-kata-sev) deploy_kata "sev" ;;
        deploy-kata-snp) deploy_kata "snp" ;;
        deploy-kata-tdx) deploy_kata "tdx" ;;
        deploy-kata-garm) deploy_kata "garm" ;;
        run-tests) run_tests ;;
        cleanup-sev) cleanup "sev" ;;
        cleanup-snp) cleanup "snp" ;;
        cleanup-tdx) cleanup "tdx" ;;
        cleanup-garm) cleanup "garm" ;;
        delete-cluster) cleanup "aks" ;;
        *) >&2 echo "Invalid argument"; exit 2 ;;
    esac
}

main "$@"
