#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

DEBUG="${DEBUG:-}"
[ -n "$DEBUG" ] && set -x

kubernetes_dir="$(dirname "$(readlink -f "$0")")"
source "${kubernetes_dir}/../../gha-run-k8s-common.sh"
# shellcheck disable=2154
tools_dir="${repo_root_dir}/tools"
kata_tarball_dir="${2:-kata-artifacts}"

DOCKER_REGISTRY=${DOCKER_REGISTRY:-quay.io}
DOCKER_REPO=${DOCKER_REPO:-kata-containers/kata-deploy-ci}
DOCKER_TAG=${DOCKER_TAG:-kata-containers-latest}
KATA_DEPLOY_WAIT_TIMEOUT=${KATA_DEPLOY_WAIT_TIMEOUT:-10m}
SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT=${SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT:-5m}
KATA_HYPERVISOR=${KATA_HYPERVISOR:-qemu}
KUBERNETES="${KUBERNETES:-}"
SNAPSHOTTER="${SNAPSHOTTER:-}"
export AUTO_GENERATE_POLICY="${AUTO_GENERATE_POLICY:-no}"
export TEST_CLUSTER_NAMESPACE="${TEST_CLUSTER_NAMESPACE:-kata-containers-k8s-tests}"

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

	cleanup_kata_deploy || true

	set_default_cluster_namespace

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

	if [ "${KATA_HYPERVISOR}" = "qemu" ]; then
		yq write -i "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" 'spec.template.spec.containers[0].env[6].value' "image initrd kernel default_vcpus"
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

	# Enable auto-generated policy for CI images that support policy.
	#
	# TODO: enable testing auto-generated policy for other types of hosts too.
	[ "${KATA_HOST_OS}" = "cbl-mariner" ] && export AUTO_GENERATE_POLICY="yes"

	set_test_cluster_namespace

	pushd "${kubernetes_dir}"
	bash setup.sh
	if [[ "${KATA_HYPERVISOR}" = "dragonball" ]] && [[ "${SNAPSHOTTER}" = "devmapper" ]] || [[ "${KATA_HYPERVISOR}" = "cloud-hypervisor" ]] && [[ "${SNAPSHOTTER}" = "devmapper" ]]; then
		# cloud-hypervisor runtime-rs issue is https://github.com/kata-containers/kata-containers/issues/9034
		echo "Skipping tests for $KATA_HYPERVISOR using devmapper"
	else
		bash run_kubernetes_tests.sh
	fi
	popd
}

function cleanup_kata_deploy() {
	ensure_yq

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
	delete_test_cluster_namespace

	cleanup_kata_deploy
}

function deploy_snapshotter() {
	echo "::group::Deploying ${SNAPSHOTTER}"
	case ${SNAPSHOTTER} in
		nydus) deploy_nydus_snapshotter ;;
		*) >&2 echo "${SNAPSHOTTER} flavour is not supported"; exit 2 ;;
	esac	
	echo "::endgroup::"
}

function cleanup_snapshotter() {
	echo "::group::Cleanuping ${SNAPSHOTTER}"
	case ${SNAPSHOTTER} in
		nydus) cleanup_nydus_snapshotter ;;
		*) >&2 echo "${SNAPSHOTTER} flavour is not supported"; exit 2 ;;
	esac
	echo "::endgroup::"
}

function deploy_nydus_snapshotter() {
    echo "::group::deploy_nydus_snapshotter"
	ensure_yq

	local nydus_snapshotter_install_dir="/tmp/nydus-snapshotter"
	local nydus_snapshotter_local_build="false"
	# At the time of writing, a container image for nydus snapshotter only supports x86_64
	# So, we need to build the image locally for other architectures
	if [ "$(uname -m)" != "x86_64" ]; then
		nydus_snapshotter_local_build="true"
		nydus_snapshotter_image="localhost:5000/nydus-snapshotter:latest"
		# Check if a local registry is running
		if ! ss -ntlp | grep -q 5000; then
			echo "Local registry is not running"
			docker run -d -p 5000:5000 --name local-registry registry:2.8.1
		fi
	fi
	if [ -d "${nydus_snapshotter_install_dir}" ]; then
		rm -rf "${nydus_snapshotter_install_dir}"
	fi
	mkdir -p "${nydus_snapshotter_install_dir}"
	nydus_snapshotter_url=$(get_from_kata_deps "externals.nydus-snapshotter.url")
	nydus_snapshotter_version=$(get_from_kata_deps "externals.nydus-snapshotter.version")
	git clone -b "${nydus_snapshotter_version}" "${nydus_snapshotter_url}" "${nydus_snapshotter_install_dir}"

	pushd "$nydus_snapshotter_install_dir"
	if [ "${nydus_snapshotter_local_build}" == "true" ]; then
		# Build and push a nydus snapshotter image locally
		make build
		cp bin/* misc/snapshotter/
		pushd misc/snapshotter/
		nydus_version=$(get_from_kata_deps "externals.nydus.version")
		docker build --build-arg NYDUS_VER="${nydus_version}" -t "${nydus_snapshotter_image}" .
		docker push "${nydus_snapshotter_image}"
		popd
	fi
	if [ "${PULL_TYPE}" == "guest-pull" ]; then
		# Enable guest pull feature in nydus snapshotter
		yq write -i misc/snapshotter/base/nydus-snapshotter.yaml 'data.FS_DRIVER' "proxy" --style=double
	else
		>&2 echo "Invalid pull type"; exit 2 
	fi
	
	# Disable to read snapshotter config from configmap
	yq write -i misc/snapshotter/base/nydus-snapshotter.yaml 'data.ENABLE_CONFIG_FROM_VOLUME' "false" --style=double
	# Enable to run snapshotter as a systemd service
	yq write -i misc/snapshotter/base/nydus-snapshotter.yaml 'data.ENABLE_SYSTEMD_SERVICE' "true" --style=double
	# Enable "runtime specific snapshotter" feature in containerd when configuring containerd for snapshotter
	yq write -i misc/snapshotter/base/nydus-snapshotter.yaml 'data.ENABLE_RUNTIME_SPECIFIC_SNAPSHOTTER' "true" --style=double
	if [ "${nydus_snapshotter_local_build}" == "true" ]; then
		# Replace the image with the local image
		yq write -i --doc=1 misc/snapshotter/base/nydus-snapshotter.yaml 'spec.template.spec.containers[*].image' "${nydus_snapshotter_image}"
	fi

	# Deploy nydus snapshotter as a daemonset
	kubectl create -f "misc/snapshotter/nydus-snapshotter-rbac.yaml"
	if [ "${KUBERNETES}" = "k3s" ]; then
		kubectl apply -k "misc/snapshotter/overlays/k3s"
	else
		kubectl apply -f "misc/snapshotter/base/nydus-snapshotter.yaml"
	fi
	popd

	kubectl rollout status daemonset nydus-snapshotter -n nydus-system --timeout ${SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT}
	
	echo "::endgroup::"
	echo "::group::nydus snapshotter logs"
	pods_name=$(kubectl get pods --selector=app=nydus-snapshotter -n nydus-system -o=jsonpath='{.items[*].metadata.name}')
	kubectl logs ${pods_name} -n nydus-system
	kubectl describe pod ${pods_name} -n nydus-system
	echo "::endgroup::"
}

function cleanup_nydus_snapshotter() {
	echo "cleanup_nydus_snapshotter"
	local nydus_snapshotter_install_dir="/tmp/nydus-snapshotter"
	if [ ! -d "${nydus_snapshotter_install_dir}" ]; then
		>&2 echo "nydus snapshotter dir not found"
		exit 1
	fi
	
	pushd "$nydus_snapshotter_install_dir"

	if [ "${KUBERNETES}" = "k3s" ]; then
		kubectl delete -k "misc/snapshotter/overlays/k3s"
	else
		kubectl delete -f "misc/snapshotter/base/nydus-snapshotter.yaml"
	fi
	sleep 180s
	kubectl delete -f "misc/snapshotter/nydus-snapshotter-rbac.yaml"
	popd
	sleep 30s

	rm -rf "${nydus_snapshotter_install_dir}"
	echo "::endgroup::"
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
		install-kata-tools) install_kata_tools ;;
		install-kubectl) install_kubectl ;;
		get-cluster-credentials) get_cluster_credentials ;;
		deploy-kata-aks) deploy_kata "aks" ;;
		deploy-kata-kcli) deploy_kata "kcli" ;;
		deploy-kata-kubeadm) deploy_kata "kubeadm" ;;
		deploy-kata-sev) deploy_kata "sev" ;;
		deploy-kata-snp) deploy_kata "snp" ;;
		deploy-kata-tdx) deploy_kata "tdx" ;;
		deploy-kata-garm) deploy_kata "garm" ;;
		deploy-kata-zvsi) deploy_kata "zvsi" ;;
		deploy-snapshotter) deploy_snapshotter ;;
		run-tests) run_tests ;;
		run-tests-kcli) run_tests "kcli" ;;
		cleanup-kcli) cleanup "kcli" ;;
		cleanup-sev) cleanup "sev" ;;
		cleanup-snp) cleanup "snp" ;;
		cleanup-tdx) cleanup "tdx" ;;
		cleanup-garm) cleanup "garm" ;;
		cleanup-zvsi) cleanup "zvsi" ;;
		cleanup-snapshotter) cleanup_snapshotter ;;
		delete-cluster) cleanup "aks" ;;
		delete-cluster-kcli) delete_cluster_kcli ;;
		*) >&2 echo "Invalid argument"; exit 2 ;;
	esac
}

main "$@"
