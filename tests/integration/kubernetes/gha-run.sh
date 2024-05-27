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
# shellcheck disable=1091
source "${kubernetes_dir}/confidential_kbs.sh"
# shellcheck disable=2154
tools_dir="${repo_root_dir}/tools"
kata_tarball_dir="${2:-kata-artifacts}"

DOCKER_REGISTRY=${DOCKER_REGISTRY:-quay.io}
DOCKER_REPO=${DOCKER_REPO:-kata-containers/kata-deploy-ci}
DOCKER_TAG=${DOCKER_TAG:-kata-containers-latest}
KATA_DEPLOY_WAIT_TIMEOUT=${KATA_DEPLOY_WAIT_TIMEOUT:-600}
SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT=${SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT:-8m}
KATA_HYPERVISOR=${KATA_HYPERVISOR:-qemu}
KBS=${KBS:-false}
KBS_INGRESS=${KBS_INGRESS:-}
KUBERNETES="${KUBERNETES:-}"
SNAPSHOTTER="${SNAPSHOTTER:-}"
HTTPS_PROXY="${HTTPS_PROXY:-${https_proxy:-}}"
NO_PROXY="${NO_PROXY:-${no_proxy:-}}"
PULL_TYPE="${PULL_TYPE:-default}"
export AUTO_GENERATE_POLICY="${AUTO_GENERATE_POLICY:-no}"
export TEST_CLUSTER_NAMESPACE="${TEST_CLUSTER_NAMESPACE:-kata-containers-k8s-tests}"
export GENPOLICY_PULL_METHOD="${GENPOLICY_PULL_METHOD:-oci-distribution-client}"

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
			sudo cp /var/lib/rancher/k3s/agent/etc/containerd/config.toml "${containerd_config_file}"
			;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	# We're not using this with baremetal machines, so we're fine on cutting
	# corners here and just append this to the configuration file.
	cat<<EOF | sudo tee -a "${containerd_config_file}"
[plugins."io.containerd.snapshotter.v1.devmapper"]
  pool_name = "contd-thin-pool"
  base_image_size = "4096MB"
EOF

	case "${KUBERNETES}" in
		k3s)
			sudo sed -i -e 's/snapshotter = "overlayfs"/snapshotter = "devmapper"/g' "${containerd_config_file}"
			sudo systemctl restart k3s ;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	sleep 60s
	sudo cat "${containerd_config_file}"

	if [ "${KUBERNETES}" = 'k3s' ]
	then
		local ctr_dm_status
		local result

		ctr_dm_status=$(sudo ctr \
			--address '/run/k3s/containerd/containerd.sock' \
			plugins ls |\
			awk '$2 ~ /^devmapper$/ { print $0 }' || true)

		result=$(echo "$ctr_dm_status" | awk '{print $4}' || true)

		[ "$result" = 'ok' ] || die "k3s containerd device mapper not configured: '$ctr_dm_status'"
	fi

	info "devicemapper (DM) devices"
	sudo dmsetup ls --tree
	sudo dmsetup status -v
}

function configure_snapshotter() {
	echo "::group::Configuring ${SNAPSHOTTER}"

	case "${SNAPSHOTTER}" in
		devmapper) configure_devmapper ;;
		*) >&2 echo "${SNAPSHOTTER} flavour is not supported"; exit 2 ;;
	esac

	echo "::endgroup::"
}

function delete_coco_kbs() {
	kbs_k8s_delete
}

# Deploy the CoCo KBS in Kubernetes
#
# Environment variables:
#	KBS_INGRESS - (optional) specify the ingress implementation to expose the
#	              service externally
#
function deploy_coco_kbs() {
	kbs_k8s_deploy "$KBS_INGRESS"
}

function deploy_kata() {
	platform="${1}"
	ensure_yq

	[ "$platform" = "kcli" ] && \
	export KUBECONFIG="$HOME/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	if [ "${K8S_TEST_HOST_TYPE}" = "baremetal" ]; then
		cleanup_kata_deploy || true
	fi

	set_default_cluster_namespace

	sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"

	# Enable debug for Kata Containers
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
	  'spec.template.spec.containers[0].env[1].value' \
	  --tag '!!str' "true"
	# Create the runtime class only for the shim that's being tested
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
	  'spec.template.spec.containers[0].env[2].value' \
	  "${KATA_HYPERVISOR}"
	# Set the tested hypervisor as the default `kata` shim
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
	  'spec.template.spec.containers[0].env[3].value' \
	  "${KATA_HYPERVISOR}"
	# Let the `kata-deploy` script take care of the runtime class creation / removal
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
	  'spec.template.spec.containers[0].env[4].value' \
	  --tag '!!str' "true"
	# Let the `kata-deploy` create the default `kata` runtime class
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
	  'spec.template.spec.containers[0].env[5].value' \
	  --tag '!!str' "true"
	# Enable 'default_vcpus' hypervisor annotation
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
	  'spec.template.spec.containers[0].env[6].value' \
	  "default_vcpus"

	if [ -n "${SNAPSHOTTER}" ]; then
		yq write -i \
		  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
		  'spec.template.spec.containers[0].env[7].value' \
		  "${KATA_HYPERVISOR}:${SNAPSHOTTER}"
	fi

	if [ "${KATA_HOST_OS}" = "cbl-mariner" ]; then
		yq write -i \
		  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
		  'spec.template.spec.containers[0].env[6].value' \
		  "initrd kernel default_vcpus"
		yq write -i \
		  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
		  'spec.template.spec.containers[0].env[+].name' \
		  "HOST_OS"
		yq write -i \
		  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
		  'spec.template.spec.containers[0].env[-1].value' \
		  "${KATA_HOST_OS}"
	fi

	if [ "${KATA_HYPERVISOR}" = "qemu" ]; then
		yq write -i \
		  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
		  'spec.template.spec.containers[0].env[6].value' \
		  "image initrd kernel default_vcpus"
	fi

	if [ "${KATA_HYPERVISOR}" = "qemu-tdx" ]; then
		yq write -i \
		  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
		  'spec.template.spec.containers[0].env[8].value' \
		  "${HTTPS_PROXY}"

		yq write -i \
		  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
		  'spec.template.spec.containers[0].env[9].value' \
		  "${NO_PROXY}"
	fi

	echo "::group::Final kata-deploy.yaml that is used in the test"
	cat "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" || die "Failed to setup the tests image"
	echo "::endgroup::"

	kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
	case "${KUBERNETES}" in
		k0s) kubectl apply -k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/k0s" ;;
		k3s) kubectl apply -k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/k3s" ;;
		rke2) kubectl apply -k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/rke2" ;;
		*) kubectl apply -f "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	esac

	local cmd="kubectl -n kube-system get -l name=kata-deploy pod 2>/dev/null | grep '\<Running\>'"
	waitForProcess "${KATA_DEPLOY_WAIT_TIMEOUT}" 10 "$cmd"

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

function install_kbs_client() {
	kbs_install_cli
}

function uninstall_kbs_client() {
	kbs_uninstall_cli
}

function run_tests() {
	ensure_yq
	platform="${1:-}"

	[ "$platform" = "kcli" ] && \
		export KUBECONFIG="$HOME/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	# Enable auto-generated policy for CI images that support policy
	# and enable cri plugin in containerd config.
	# TODO: enable testing auto-generated policy for other types of hosts too.

	if [ "${KATA_HOST_OS}" = "cbl-mariner" ]; then

		export AUTO_GENERATE_POLICY="yes"

		# set default containerd config
		sudo containerd config default | sudo tee /etc/containerd/config.toml > /dev/null
		echo "containerd config has been set to default"
		sudo systemctl restart containerd && sudo systemctl is-active containerd
	fi

	set_test_cluster_namespace

	pushd "${kubernetes_dir}"
	bash setup.sh

	# In case of running on Github workflow it needs to save the start time
	# on the environment variables file so that the variable is exported on
	# next workflow steps.
	if [ -n "${GITHUB_ENV:-}" ]; then
		start_time=$(date '+%Y-%m-%d %H:%M:%S')
		export start_time
		echo "start_time=${start_time}" >> "$GITHUB_ENV"
	fi

	if [[ "${KATA_HYPERVISOR}" = "cloud-hypervisor" ]] && [[ "${SNAPSHOTTER}" = "devmapper" ]]; then
		if [ -n "$GITHUB_ENV" ]; then
			KATA_TEST_VERBOSE=true
			export KATA_TEST_VERBOSE
			echo "KATA_TEST_VERBOSE=${KATA_TEST_VERBOSE}" >> "$GITHUB_ENV"
		fi
	fi

	if [[ "${KATA_HYPERVISOR}" = "dragonball" ]] && [[ "${SNAPSHOTTER}" = "devmapper" ]]; then
		echo "Skipping tests for $KATA_HYPERVISOR using devmapper"
	else
		bash run_kubernetes_tests.sh
	fi
	popd
}

function collect_artifacts() {
	if [ -z "${start_time:-}" ]; then
		warn "tests start time is not defined. Cannot gather journal information"
		return
	fi

	local artifacts_dir="/tmp/artifacts"
	if [ -d "${artifacts_dir}" ]; then
		rm -rf "${artifacts_dir}"
	fi
	mkdir -p "${artifacts_dir}"
	info "Collecting artifacts using ${KATA_HYPERVISOR} hypervisor"
	local journalctl_log_filename="journalctl-$RANDOM.log"
	local journalctl_log_path="${artifacts_dir}/${journalctl_log_filename}"
	sudo journalctl --since="$start_time" > "${journalctl_log_path}"

	local k3s_dir='/var/lib/rancher/k3s/agent'

	if [ -d "$k3s_dir" ]
	then
		info "Collecting k3s artifacts"

		local -a files=()

		files+=('etc/containerd/config.toml')
		files+=('etc/containerd/config.toml.tmpl')

		files+=('containerd/containerd.log')

		# Add any rotated containerd logs
		files+=( $(sudo find \
			"${k3s_dir}/containerd/" \
			-type f \
			-name 'containerd*\.log\.gz') )

		local file

		for file in "${files[@]}"
		do
			local path="$k3s_dir/$file"
			sudo [ ! -e "$path" ] && continue

			local encoded
			encoded=$(echo "$path" | tr '/' '-' | sed 's/^-//g')

			local from="$path"

			local to

			to="${artifacts_dir}/${encoded}"

			if [[ $path = *.gz ]]
			then
				sudo cp "$from" "$to"
			else
				to="${to}.gz"
				sudo gzip -c "$from" > "$to"
			fi

			info "  Collected k3s file '$from' to '$to'"
		done
	fi
}

function cleanup_kata_deploy() {
	ensure_yq

	case "${KUBERNETES}" in
		k0s)
			deploy_spec="-k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/k0s""
			cleanup_spec="-k "${tools_dir}/packaging/kata-deploy/kata-cleanup/overlays/k0s""
			;;
		k3s)
			deploy_spec="-k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/k3s""
			cleanup_spec="-k "${tools_dir}/packaging/kata-deploy/kata-cleanup/overlays/k3s""
			;;
		rke2)
			deploy_spec="-k "${tools_dir}/packaging/kata-deploy/kata-deploy/overlays/rke2""
			cleanup_spec="-k "${tools_dir}/packaging/kata-deploy/kata-cleanup/overlays/rke2""
			;;
		*)
			deploy_spec="-f "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml""
			cleanup_spec="-f "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml""
			;;
	esac

	# shellcheck disable=2086
	kubectl delete ${deploy_spec}
	kubectl -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod

	# Let the `kata-deploy` script take care of the runtime class creation / removal
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" \
	  'spec.template.spec.containers[0].env[4].value' \
	  --tag '!!str' "true"
	# Create the runtime class only for the shim that's being tested
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" \
	  'spec.template.spec.containers[0].env[2].value' \
	  "${KATA_HYPERVISOR}"
	# Set the tested hypervisor as the default `kata` shim
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" \
	  'spec.template.spec.containers[0].env[3].value' \
	  "${KATA_HYPERVISOR}"
	# Let the `kata-deploy` create the default `kata` runtime class
	yq write -i \
	  "${tools_dir}/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" \
	  'spec.template.spec.containers[0].env[5].value' \
	  --tag '!!str' "true"

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
		delete_cluster "${test_type}"
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
	if [ -d "${nydus_snapshotter_install_dir}" ]; then
		rm -rf "${nydus_snapshotter_install_dir}"
	fi
	mkdir -p "${nydus_snapshotter_install_dir}"
	nydus_snapshotter_url=$(get_from_kata_deps "externals.nydus-snapshotter.url")
	nydus_snapshotter_version=$(get_from_kata_deps "externals.nydus-snapshotter.version")
	git clone -b "${nydus_snapshotter_version}" "${nydus_snapshotter_url}" "${nydus_snapshotter_install_dir}"

	pushd "$nydus_snapshotter_install_dir"
	if [ "${K8S_TEST_HOST_TYPE}" = "baremetal" ]; then
		cleanup_nydus_snapshotter || true
	fi
	if [ "${PULL_TYPE}" == "guest-pull" ]; then
		# Enable guest pull feature in nydus snapshotter
		yq write -i \
		  misc/snapshotter/base/nydus-snapshotter.yaml \
		  'data.FS_DRIVER' \
		  "proxy" --style=double
	else
		>&2 echo "Invalid pull type"; exit 2
	fi

	# Disable to read snapshotter config from configmap
	yq write -i \
	  misc/snapshotter/base/nydus-snapshotter.yaml \
	  'data.ENABLE_CONFIG_FROM_VOLUME' \
	  "false" --style=double
	# Enable to run snapshotter as a systemd service
	yq write -i \
	  misc/snapshotter/base/nydus-snapshotter.yaml \
	  'data.ENABLE_SYSTEMD_SERVICE' \
	  "true" --style=double
	# Enable "runtime specific snapshotter" feature in containerd when configuring containerd for snapshotter
	yq write -i \
	  misc/snapshotter/base/nydus-snapshotter.yaml \
	  'data.ENABLE_RUNTIME_SPECIFIC_SNAPSHOTTER' \
	  "true" --style=double

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
	kubectl logs "${pods_name}" -n nydus-system
	kubectl describe pod "${pods_name}" -n nydus-system
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
	kubectl get namespace nydus-system -o json | jq 'del(.spec.finalizers)' | kubectl replace --raw "/api/v1/namespaces/nydus-system/finalize" -f - || true
	popd
	sleep 30s
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
		deploy-coco-kbs) deploy_coco_kbs ;;
		deploy-k8s) deploy_k8s ;;
		install-bats) install_bats ;;
		install-kata-tools) install_kata_tools ;;
		install-kbs-client) install_kbs_client ;;
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
		collect-artifacts) collect_artifacts ;;
		cleanup-kcli) cleanup "kcli" ;;
		cleanup-sev) cleanup "sev" ;;
		cleanup-snp) cleanup "snp" ;;
		cleanup-tdx) cleanup "tdx" ;;
		cleanup-garm) cleanup "garm" ;;
		cleanup-zvsi) cleanup "zvsi" ;;
		cleanup-snapshotter) cleanup_snapshotter ;;
		delete-coco-kbs) delete_coco_kbs ;;
		delete-cluster) cleanup "aks" ;;
		delete-cluster-kcli) delete_cluster_kcli ;;
		uninstall-kbs-client) uninstall_kbs_client ;;
		*) >&2 echo "Invalid argument"; exit 2 ;;
	esac
}

main "$@"
