#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

DEBUG="${DEBUG:-}"
[[ -n "${DEBUG}" ]] && set -x

kubernetes_dir="${kubernetes_dir:-$(dirname "$(readlink -f "$0")")}"
source "${kubernetes_dir}/../../gha-run-k8s-common.sh"
# shellcheck disable=1091
source "${kubernetes_dir}/confidential_kbs.sh"
# shellcheck disable=2154
tools_dir="${repo_root_dir}/tools"
kata_tarball_dir="${2:-kata-artifacts}"

export DOCKER_REGISTRY="${DOCKER_REGISTRY:-quay.io}"
export DOCKER_REPO="${DOCKER_REPO:-kata-containers/kata-deploy-ci}"
export DOCKER_TAG="${DOCKER_TAG:-kata-containers-latest}"
export SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT="${SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT:-8m}"
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
export CONTAINER_RUNTIME="${CONTAINER_RUNTIME:-containerd}"
export KBS="${KBS:-false}"
export KBS_INGRESS="${KBS_INGRESS:-}"
export KUBERNETES="${KUBERNETES:-}"
export SNAPSHOTTER="${SNAPSHOTTER:-}"
export ITA_KEY="${ITA_KEY:-}"
export HTTPS_PROXY="${HTTPS_PROXY:-${https_proxy:-}}"
export NO_PROXY="${NO_PROXY:-${no_proxy:-}}"
export PULL_TYPE="${PULL_TYPE:-default}"
export TEST_CLUSTER_NAMESPACE="${TEST_CLUSTER_NAMESPACE:-kata-containers-k8s-tests}"
export GENPOLICY_PULL_METHOD="${GENPOLICY_PULL_METHOD:-oci-distribution}"
export TARGET_ARCH="${TARGET_ARCH:-x86_64}"
export RUNS_ON_AKS="${RUNS_ON_AKS:-false}"

function configure_devmapper() {
	sudo mkdir -p /var/lib/containerd/devmapper
	sudo truncate --size 30G /var/lib/containerd/devmapper/data-disk.img
	sudo truncate --size 2G  /var/lib/containerd/devmapper/meta-disk.img

	# Allocate loop devices dynamically to avoid conflicts with pre-existing ones
	# (e.g. snap loop mounts on ubuntu-24.04).
	local loop_data loop_meta
	loop_data=$(sudo losetup --find --show /var/lib/containerd/devmapper/data-disk.img)
	loop_meta=$(sudo losetup --find --show /var/lib/containerd/devmapper/meta-disk.img)
	info "devmapper: data=${loop_data} meta=${loop_meta}"

	# Persist the loop device mapping across reboots / containerd restarts.
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
ExecStart=-/sbin/losetup ${loop_data} /var/lib/containerd/devmapper/data-disk.img
ExecStart=-/sbin/losetup ${loop_meta} /var/lib/containerd/devmapper/meta-disk.img
[Install]
WantedBy=local-fs.target
EOF

	sudo systemctl daemon-reload
	sudo systemctl enable containerd-devmapper

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
	local data_sectors
	data_sectors=$(sudo blockdev --getsz "${loop_data}")
	sudo dmsetup create contd-thin-pool \
		--table "0 ${data_sectors} thin-pool ${loop_meta} ${loop_data} 512 32768 1 skip_block_zeroing"

	case "${KUBERNETES}" in
		k3s)
			containerd_config_file="/var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl"
			sudo cp /var/lib/rancher/k3s/agent/etc/containerd/config.toml "${containerd_config_file}"
			;;
		kubeadm|vanilla)
			containerd_config_file="/etc/containerd/config.toml"
			;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	# We need to use tomlq to update the containerd config with the devmapper configuration,
	# as it's a more complex update that involves adding new entries and modifying existing ones
	# for two different containerd versions.
	install_tomlq

	containerd_arch="$(uname -m)"
	case "${containerd_arch}" in
		x86_64) containerd_arch="amd64" ;;
		aarch64|arm64) containerd_arch="arm64" ;;
	esac

	echo "Updating containerd config with tomlq..."
	config_tmp_file="$(sudo mktemp)"
	sudo cat "${containerd_config_file}" | tomlq -t --arg platform "linux/${containerd_arch}" '
		.plugins["io.containerd.snapshotter.v1.devmapper"].pool_name = "contd-thin-pool"
		| .plugins["io.containerd.snapshotter.v1.devmapper"].base_image_size = "10240MB"
		| .plugins["io.containerd.transfer.v1.local"].unpack_config =
			[((.plugins["io.containerd.transfer.v1.local"].unpack_config[0] // {}) + {platform: $platform, snapshotter: "devmapper"})]
		| if .version == 3 then
			.plugins["io.containerd.cri.v1.images"].snapshotter = "devmapper"
		  else
			.plugins["io.containerd.grpc.v1.cri"].containerd.snapshotter = "devmapper"
		  end
	' | sudo tee "${config_tmp_file}" > /dev/null
	sudo mv "${config_tmp_file}" "${containerd_config_file}"

	# We only need tomlq for this configuration.
	# yq, installed by install_tomlq, might cause an issue with go-based yq used by CI.
	# So we uninstall tomlq to remove the yq from PATH and avoid any potential conflict.
	uninstall_tomlq

	case "${KUBERNETES}" in
		k3s)
			sudo systemctl restart k3s ;;
		kubeadm|vanilla)
			sudo systemctl restart containerd ;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	sleep 60s
	sudo cat "${containerd_config_file}"

	if [[ "${KUBERNETES}" = 'k3s' ]]
	then
		local ctr_dm_status
		local result

		ctr_dm_status=$(sudo ctr \
			--address '/run/k3s/containerd/containerd.sock' \
			plugins ls |\
			awk '$2 ~ /^devmapper$/ { print $0 }' || true)

		result=$(echo "${ctr_dm_status}" | awk '{print $4}' || true)

		[[ "${result}" = 'ok' ]] || die "k3s containerd device mapper not configured: '${ctr_dm_status}'"
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

# Pre-label the node with CPU virtualisation feature labels so that the
# kata-deploy DaemonSet (which has a required nodeAffinity for these labels
# set by node-feature-discovery) can be scheduled immediately after Helm
# install, without waiting for NFD to detect and apply the labels.
# This is needed when devmapper is the global CRI snapshotter: NFD pod image
# pulls are slower with devmapper, causing kata-deploy scheduling to miss the
# 900-second KATA_DEPLOY_WAIT_TIMEOUT.
function prelabel_node_for_kata_deploy() {
	local node
	node=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')

	if lsmod | grep -q 'kvm_amd'; then
		info "AMD SVM detected via kvm_amd — labelling node ${node}"
		kubectl label node "${node}" \
			feature.node.kubernetes.io/cpu-cpuid.SVM=true --overwrite
	elif lsmod | grep -q 'kvm_intel'; then
		info "Intel VMX detected via kvm_intel — labelling node ${node}"
		kubectl label node "${node}" \
			feature.node.kubernetes.io/cpu-cpuid.VMX=true --overwrite
	else
		warn "Neither kvm_amd nor kvm_intel loaded; kata-deploy scheduling may be delayed"
	fi
}

# Pre-pull the kata-deploy and NFD images into the containerd k8s.io namespace
# using the devmapper snapshotter, so the kata-deploy DaemonSet pod can start
# without waiting for slow devmapper image pulls inside the 900 s
# KATA_DEPLOY_WAIT_TIMEOUT window.
function prepull_kata_deploy_images() {
	local kata_deploy_image="${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}"

	info "Pre-pulling kata-deploy image (devmapper): ${kata_deploy_image}"
	sudo ctr -n k8s.io images pull --snapshotter devmapper "${kata_deploy_image}"

	# Resolve NFD image version from the helm dependency lock file
	pushd "${helm_chart_dir}" > /dev/null
	helm dependency update 2>&1 | tail -5 || true
	popd > /dev/null

	if [[ -f "${helm_chart_dir}/Chart.lock" ]]; then
		local nfd_version
		nfd_version=$(grep -A3 "name: node-feature-discovery" "${helm_chart_dir}/Chart.lock" \
			| grep "version:" | awk '{print $2}')
		if [[ -n "${nfd_version}" ]]; then
			local nfd_image="registry.k8s.io/nfd/node-feature-discovery:v${nfd_version}"
			info "Pre-pulling NFD image (devmapper): ${nfd_image}"
			sudo ctr -n k8s.io images pull --snapshotter devmapper "${nfd_image}" || true
		fi
	fi
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
	kbs_k8s_deploy "${KBS_INGRESS}"
}

function deploy_kata() {
	platform="${1:-}"

	[[ "${platform}" = "kcli" ]] && \
	export KUBECONFIG="${HOME}/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	if [[ "${K8S_TEST_HOST_TYPE}" = "baremetal"* ]]; then
		cleanup_kata_deploy || true
	fi

	set_default_cluster_namespace

	# Workaround to avoid modifying the workflow yaml files
	if is_tdx_hypervisor "${KATA_HYPERVISOR}" || is_snp_hypervisor "${KATA_HYPERVISOR}" || is_confidential_gpu_hypervisor "${KATA_HYPERVISOR}"; then
		USE_EXPERIMENTAL_SETUP_SNAPSHOTTER=true
		SNAPSHOTTER="nydus"
		EXPERIMENTAL_FORCE_GUEST_PULL=false
	fi

	ANNOTATIONS="default_vcpus"
	if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
		ANNOTATIONS="image kernel default_vcpus cc_init_data"
	fi
	if [[ "${KATA_HYPERVISOR}" = "qemu" ]]; then
		ANNOTATIONS="image initrd kernel default_vcpus"
	fi

	SNAPSHOTTER_HANDLER_MAPPING=""
	if [[ -n "${SNAPSHOTTER}" ]]; then
		SNAPSHOTTER_HANDLER_MAPPING="${KATA_HYPERVISOR}:${SNAPSHOTTER}"
	fi

	PULL_TYPE_MAPPING=""
	if [[ "${PULL_TYPE}" != "default" ]]; then
		PULL_TYPE_MAPPING="${KATA_HYPERVISOR}:${PULL_TYPE}"
	fi

	HOST_OS=""
	if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
		HOST_OS="${KATA_HOST_OS}"
	fi

	# nydus and erofs are always deployed by kata-deploy; set this unconditionally
	# based on the snapshotter so that all architectures and hypervisors work
	# without needing per-workflow USE_EXPERIMENTAL_SETUP_SNAPSHOTTER overrides.
	EXPERIMENTAL_SETUP_SNAPSHOTTER=""
	case "${SNAPSHOTTER}" in
		nydus|erofs) EXPERIMENTAL_SETUP_SNAPSHOTTER="${SNAPSHOTTER}" ;;
		*) ;;
	esac

	EXPERIMENTAL_FORCE_GUEST_PULL="${EXPERIMENTAL_FORCE_GUEST_PULL:-}"

	export HELM_K8S_DISTRIBUTION="${KUBERNETES}"
	export HELM_IMAGE_REFERENCE="${DOCKER_REGISTRY}/${DOCKER_REPO}"
	export HELM_IMAGE_TAG="${DOCKER_TAG}"
	export HELM_DEBUG="true"
	export HELM_SHIMS="${KATA_HYPERVISOR}"
	export HELM_DEFAULT_SHIM="${KATA_HYPERVISOR}"
	export HELM_CREATE_DEFAULT_RUNTIME_CLASS="true"
	export HELM_ALLOWED_HYPERVISOR_ANNOTATIONS="${ANNOTATIONS}"
	export HELM_SNAPSHOTTER_HANDLER_MAPPING="${SNAPSHOTTER_HANDLER_MAPPING}"
	export HELM_AGENT_HTTPS_PROXY="${HTTPS_PROXY}"
	export HELM_AGENT_NO_PROXY="${NO_PROXY}"
	export HELM_PULL_TYPE_MAPPING="${PULL_TYPE_MAPPING}"
	export HELM_EXPERIMENTAL_SETUP_SNAPSHOTTER="${EXPERIMENTAL_SETUP_SNAPSHOTTER}"
	export HELM_EXPERIMENTAL_FORCE_GUEST_PULL="${EXPERIMENTAL_FORCE_GUEST_PULL}"
	export HELM_HOST_OS="${HOST_OS}"
	helm_helper
}

function install_kbs_client() {
	kbs_install_cli
}

function uninstall_kbs_client() {
	kbs_uninstall_cli
}

function run_tests() {
	if [[ "${K8S_TEST_HOST_TYPE}" = "baremetal"* ]]; then
		# Baremetal self-hosted runners end up accumulating way too much log
		# and when those get displayed it's very hard to understand what's
		# part of the current run and what's something from the past coming
		# to haunt us.
		#
		# With this in mind, let's ensure we do rotate the logs on every single
		# run of the tests, as its first step.
		sudo journalctl --vacuum-time 1s --rotate
	fi

	ensure_yq
	platform="${1:-}"

	[[ "${platform}" = "kcli" ]] && \
		export KUBECONFIG="${HOME}/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	if [[ "${AUTO_GENERATE_POLICY}" = "yes" ]] && [[ "${GENPOLICY_PULL_METHOD}" = "containerd" ]]; then
		# containerd's config on the local machine (where kubectl and genpolicy are executed by CI),
		# might have been provided by a distro-specific package that disables the cri plug-in by using:
		#
		# disabled_plugins = ["cri"]
		#
		# When testing genpolicy's container image pull through containerd the cri plug-in must be
		# enabled. Therefore, use containerd's default settings instead of distro's defaults. Note that
		# the k8s test cluster nodes have their own containerd settings (created by kata-deploy),
		# independent from the local settings being created here.
		sudo containerd config default | sudo tee /etc/containerd/config.toml > /dev/null
		echo "containerd config has been set to default"
		sudo systemctl restart containerd && sudo systemctl is-active containerd

		# Allow genpolicy to access the containerd image pull APIs without sudo.
		local socket_wait_time
		local socket_sleep_time
		local cmd

		socket_wait_time=30
		socket_sleep_time=3
		cmd="sudo chmod a+rw /var/run/containerd/containerd.sock"

		waitForProcess "${socket_wait_time}" "${socket_sleep_time}" "${cmd}"
	fi

	set_test_cluster_namespace

	pushd "${kubernetes_dir}"
	bash setup.sh

	# In case of running on Github workflow it needs to save the start time
	# on the environment variables file so that the variable is exported on
	# next workflow steps.
	if [[ -n "${GITHUB_ENV:-}" ]]; then
		start_time=$(date '+%Y-%m-%d %H:%M:%S')
		export start_time
		echo "start_time=${start_time}" >> "${GITHUB_ENV}"
	fi

	if [[ "${KATA_HYPERVISOR}" = "cloud-hypervisor" ]] && [[ "${SNAPSHOTTER}" = "devmapper" ]]; then
		if [[ -n "${GITHUB_ENV}" ]]; then
			KATA_TEST_VERBOSE=true
			export KATA_TEST_VERBOSE
			echo "KATA_TEST_VERBOSE=${KATA_TEST_VERBOSE}" >> "${GITHUB_ENV}"
		fi
	fi

	if [[ "${KATA_HYPERVISOR}" = "dragonball" ]] && [[ "${SNAPSHOTTER}" = "devmapper" ]]; then
		echo "Skipping tests for ${KATA_HYPERVISOR} using devmapper"
	else
		bash "${K8STESTS}"
	fi
	popd
}

# Print a report about tests executed.
#
# Crawl over the output files found on each "reports/yyyy-mm-dd-hh:mm:ss"
# directory.
#
function report_tests() {
	report_bats_tests "${kubernetes_dir}"
}

function collect_artifacts() {
	if [[ -z "${start_time:-}" ]]; then
		warn "tests start time is not defined. Cannot gather journal information"
		return
	fi

	local artifacts_dir
	artifacts_dir="/tmp/artifacts"
	if [[ -d "${artifacts_dir}" ]]; then
		rm -rf "${artifacts_dir}"
	fi
	mkdir -p "${artifacts_dir}"
	info "Collecting artifacts using ${KATA_HYPERVISOR} hypervisor"
	local journalctl_log_filename
	local journalctl_log_path

	journalctl_log_filename="journalctl-${RANDOM}.log"
	journalctl_log_path="${artifacts_dir}/${journalctl_log_filename}"

	# As we want to call journalctl with sudo, we're safe to ignore SC2024 here
	# shellcheck disable=SC2024
	sudo journalctl --since="${start_time}" > "${journalctl_log_path}"

	local k3s_dir
	k3s_dir='/var/lib/rancher/k3s/agent'

	if [[ -d "${k3s_dir}" ]]
	then
		info "Collecting k3s artifacts"

		local -a files=()

		files+=('etc/containerd/config.toml')
		files+=('etc/containerd/config.toml.tmpl')

		files+=('containerd/containerd.log')

		# Add any rotated containerd logs
		files+=("$(sudo find "${k3s_dir}/containerd/" -type f -name 'containerd*\.log\.gz')")

		local file

		for file in "${files[@]}"
		do
			local path="${k3s_dir}/${file}"
			sudo [[ ! -e "${path}" ]] && continue

			local encoded
			encoded="$(echo "${path}" | tr '/' '-' | sed 's/^-//g')"

			local from
			local to

			from="${path}"
			to="${artifacts_dir}/${encoded}"

			if [[ ${path} = *.gz ]]
			then
				sudo cp "${from}" "${to}"
			else
				to="${to}.gz"
				# As we want to call gzip with sudo, we're safe to ignore SC2024 here
				# shellcheck disable=SC2024
				sudo gzip -c "${from}" > "${to}"
			fi

			info "  Collected k3s file '${from}' to '${to}'"
		done
	fi
}

function cleanup_kata_deploy() {
	ensure_helm

	# Do not return after deleting only the parent object cascade=foreground
	# means also wait for child/dependent object deletion
	helm uninstall kata-deploy --ignore-not-found --wait --cascade foreground --timeout 10m --namespace kube-system --debug || true

	wait_for_api_and_retry_uninstall "kata-deploy" "kube-system"
}

function cleanup() {
	platform="${1:-}"
	test_type="${2:-k8s}"
	ensure_yq

	[[ "${platform}" = "kcli" ]] && \
		export KUBECONFIG="${HOME}/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	echo "Gather information about the nodes and pods before cleaning up the node"
	get_nodes_and_pods_info

	if [[ "${platform}" = "aks" ]]; then
		delete_cluster "${test_type}"
		return
	fi

	# In case of canceling workflow manually, 'run_kubernetes_tests.sh' continues running and triggers new tests,
	# resulting in the CI being in an unexpected state. So we need kill all running test scripts before cleaning up the node.
	# See issue https://github.com/kata-containers/kata-containers/issues/9980
	delete_test_runners || true
	# Switch back to the default namespace and delete the tests one
	delete_test_cluster_namespace || true

	cleanup_kata_deploy
}

function main() {
	export KATA_HOST_OS="${KATA_HOST_OS:-}"
	export K8S_TEST_HOST_TYPE="${K8S_TEST_HOST_TYPE:-}"

	AUTO_GENERATE_POLICY="${AUTO_GENERATE_POLICY:-}"

	# Auto-generate policy on some Host types, if the caller didn't specify an AUTO_GENERATE_POLICY value.
	if [[ -z "${AUTO_GENERATE_POLICY}" ]]; then
		# https://github.com/kata-containers/kata-containers/issues/12839
		if [[ "${KATA_HOST_OS}" = "cbl-mariner" && \
			  "${KATA_HYPERVISOR}" = "clh" ]]; then
			AUTO_GENERATE_POLICY="yes"
		elif [[ "${KATA_HYPERVISOR}" = qemu-coco-dev* && \
		        ( "${TARGET_ARCH}" = "x86_64" || "${TARGET_ARCH}" = "aarch64" ) && \
		        "${PULL_TYPE}" != "experimental-force-guest-pull" ]]; then
			AUTO_GENERATE_POLICY="yes"
		elif [[ "${KATA_HYPERVISOR}" = qemu-nvidia-gpu-* ]]; then
			AUTO_GENERATE_POLICY="yes"
		fi
	fi

	info "Exporting AUTO_GENERATE_POLICY=${AUTO_GENERATE_POLICY}"
	export AUTO_GENERATE_POLICY

	action="${1:-}"

	case "${action}" in
		create-cluster) create_cluster "" ;;
		create-cluster-kcli) create_cluster_kcli ;;
		configure-snapshotter) configure_snapshotter ;;
		prelabel-node) prelabel_node_for_kata_deploy ;;
		prepull-kata-images) prepull_kata_deploy_images ;;
		deploy-coco-kbs) deploy_coco_kbs ;;
		deploy-k8s) deploy_k8s ${CONTAINER_ENGINE:-} ${CONTAINER_ENGINE_VERSION:-};;
		install-bats) install_bats ;;
		install-kata-tools) install_kata_tools "${2:-}" ;;
		install-kbs-client) install_kbs_client ;;
		get-cluster-credentials) get_cluster_credentials ;;
		deploy-kata) deploy_kata ;;
		deploy-kata-aks) deploy_kata "aks" ;;
		deploy-kata-kcli) deploy_kata "kcli" ;;
		deploy-kata-kubeadm) deploy_kata "kubeadm" ;;
		deploy-kata-garm) deploy_kata "garm" ;;
		deploy-kata-zvsi) deploy_kata "zvsi" ;;
		report-tests) report_tests ;;
		run-tests)
			K8STESTS=run_kubernetes_tests.sh
			run_tests
			;;
		run-nv-tests)
			K8STESTS=run_kubernetes_nv_tests.sh
			run_tests
			;;
		run-tests-kcli) run_tests "kcli" ;;
		collect-artifacts) collect_artifacts ;;
		cleanup) cleanup ;;
		cleanup-kcli) cleanup "kcli" ;;
		cleanup-kubeadm) cleanup "kubeadm" ;;
		cleanup-garm) cleanup "garm" ;;
		cleanup-zvsi) cleanup "zvsi" ;;
		delete-coco-kbs) delete_coco_kbs ;;
		delete-cluster) cleanup "aks" ;;
		delete-cluster-kcli) delete_cluster_kcli ;;
		uninstall-kbs-client) uninstall_kbs_client ;;
		*) >&2 echo "Invalid argument"; exit 2 ;;
	esac
}

main "$@"
