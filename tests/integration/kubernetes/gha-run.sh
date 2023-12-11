#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir="$(dirname "$(readlink -f "$0")")"
source "${kubernetes_dir}/../../gha-run-k8s-common.sh"
# shellcheck disable=2154
tools_dir="${repo_root_dir}/tools"

DOCKER_REGISTRY=${DOCKER_REGISTRY:-quay.io}
DOCKER_REPO=${DOCKER_REPO:-kata-containers/kata-deploy-ci}
DOCKER_TAG=${DOCKER_TAG:-kata-containers-latest}
KATA_DEPLOY_WAIT_TIMEOUT=${KATA_DEPLOY_WAIT_TIMEOUT:-10m}
KATA_HYPERVISOR=${KATA_HYPERVISOR:-qemu}
KUBERNETES="${KUBERNETES:-}"
SNAPSHOTTER="${SNAPSHOTTER:-}"

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

	[ "$platform" = "kcli" ] && \
	export KUBECONFIG="$HOME/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	# Ensure we're in the default namespace
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
	# Enable 'default_vcpus' hypervisor annotation
	yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[6].value' "default_vcpus"

	if [ "${KATA_HOST_OS}" = "cbl-mariner" ]; then
		yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[6].value' "initrd kernel default_vcpus"
		yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[+].name' "HOST_OS"
		yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[-1].value' "${KATA_HOST_OS}"
	fi

	echo "::group::Final kata-deploy.yaml that is used in the test"
	cat "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" || die "Failed to setup the tests image"
	echo "::endgroup::"

	kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
	if [ "${KUBERNETES}" = "k3s" ]; then
		kubectl apply -k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/k3s"
	else
		kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	fi
	kubectl -n kube-system wait --timeout="${KATA_DEPLOY_WAIT_TIMEOUT}" --for=condition=Ready -l name=kata-deploy pod

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

function run_tests() {
	platform="${1:-}"

	[ "$platform" = "kcli" ] && \
		export KUBECONFIG="$HOME/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	# Delete any spurious tests namespace that was left behind
	kubectl delete namespace kata-containers-k8s-tests &> /dev/null || true

	# Create a new namespace for the tests and switch to it
	kubectl apply -f "${kubernetes_dir}/runtimeclass_workloads/tests-namespace.yaml"
	kubectl config set-context --current --namespace=kata-containers-k8s-tests

	pushd "${kubernetes_dir}"
	bash setup.sh
	if [[ "${KATA_HYPERVISOR}" = "dragonball" ]] && [[ "${SNAPSHOTTER}" = "devmapper" ]]; then
		echo "Skipping tests for dragonball using devmapper"
	elif [[ "${KATA_HYPERVISOR}" = "cloud-hypervisor" ]]; then
		echo "Skipping tests for ${KATA_HYPERVISOR}"
	else
		bash run_kubernetes_tests.sh
	fi
	popd
}

function cleanup() {
	platform="${1}"
	test_type="${2:-k8s}"
	ensure_yq

	[ "$platform" = "kcli" ] && \
		export KUBECONFIG="$HOME/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

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

	# shellcheck disable=2086
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
	grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" || die "Failed to setup the tests image"
	# shellcheck disable=2086
	kubectl apply ${cleanup_spec}
	sleep 180s

	# shellcheck disable=2086
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
		create-cluster-kcli) create_cluster_kcli ;;
		configure-snapshotter) configure_snapshotter ;;
		setup-crio) setup_crio ;;
		deploy-k8s) deploy_k8s ;;
		install-bats) install_bats ;;
		install-kubectl) install_kubectl ;;
		get-cluster-credentials) get_cluster_credentials ;;
		deploy-kata-aks) deploy_kata "aks" ;;
		deploy-kata-kcli) deploy_kata "kcli" ;;
		deploy-kata-sev) deploy_kata "sev" ;;
		deploy-kata-snp) deploy_kata "snp" ;;
		deploy-kata-tdx) deploy_kata "tdx" ;;
		deploy-kata-garm) deploy_kata "garm" ;;
		deploy-kata-zvsi) deploy_kata "zvsi" ;;
		run-tests) run_tests ;;
		run-tests-kcli) run_tests "kcli" ;;
		cleanup-kcli) cleanup "kcli" ;;
		cleanup-sev) cleanup "sev" ;;
		cleanup-snp) cleanup "snp" ;;
		cleanup-tdx) cleanup "tdx" ;;
		cleanup-garm) cleanup "garm" ;;
		cleanup-zvsi) cleanup "zvsi" ;;
		delete-cluster) cleanup "aks" ;;
		delete-cluster-kcli) delete_cluster_kcli ;;
		*) >&2 echo "Invalid argument"; exit 2 ;;
	esac
}

main "$@"
