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
helm_chart_dir="${tools_dir}/packaging/kata-deploy/helm-chart/kata-deploy"
kata_tarball_dir="${2:-kata-artifacts}"

DOCKER_REGISTRY=${DOCKER_REGISTRY:-quay.io}
DOCKER_REPO=${DOCKER_REPO:-kata-containers/kata-deploy-ci}
DOCKER_TAG=${DOCKER_TAG:-kata-containers-latest}
KATA_DEPLOY_WAIT_TIMEOUT=${KATA_DEPLOY_WAIT_TIMEOUT:-600}
SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT=${SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT:-8m}
KATA_HYPERVISOR=${KATA_HYPERVISOR:-qemu}
CONTAINER_RUNTIME=${CONTAINER_RUNTIME:-containerd}
KBS=${KBS:-false}
KBS_INGRESS=${KBS_INGRESS:-}
KUBERNETES="${KUBERNETES:-}"
SNAPSHOTTER="${SNAPSHOTTER:-}"
ITA_KEY="${ITA_KEY:-}"
HTTPS_PROXY="${HTTPS_PROXY:-${https_proxy:-}}"
NO_PROXY="${NO_PROXY:-${no_proxy:-}}"
PULL_TYPE="${PULL_TYPE:-default}"
export AUTO_GENERATE_POLICY="${AUTO_GENERATE_POLICY:-no}"
export TEST_CLUSTER_NAMESPACE="${TEST_CLUSTER_NAMESPACE:-kata-containers-k8s-tests}"
export GENPOLICY_PULL_METHOD="${GENPOLICY_PULL_METHOD:-oci-distribution}"

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
		kubeadm)
			containerd_config_file="/etc/containerd/config.toml"
			;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	# We're not using this with baremetal machines, so we're fine on cutting
	# corners here and just append this to the configuration file.
	# Check if the "devmapper" plugin section exists in the file
	if grep -q 'plugins."io.containerd.snapshotter.v1.devmapper"' "${containerd_config_file}"; then
	    echo "devmapper section found. Updating pool_name and base_image_size..."
	    sudo sed -i '/\[plugins."io.containerd.snapshotter.v1.devmapper"\]/,/\[plugins\./ {
	        s/pool_name = ".*"/pool_name = "contd-thin-pool"/
	        s/base_image_size = ".*"/base_image_size = "4096MB"/
	    }' "${containerd_config_file}"
	else
	    echo "devmapper section not found. Appending to the config file..."
		cat<<EOF | sudo tee -a "${containerd_config_file}"
[plugins."io.containerd.snapshotter.v1.devmapper"]
  pool_name = "contd-thin-pool"
  base_image_size = "4096MB"
EOF
	fi

	case "${KUBERNETES}" in
		k3s)
			sudo sed -i -e 's/snapshotter = "overlayfs"/snapshotter = "devmapper"/g' "${containerd_config_file}"
			sudo systemctl restart k3s ;;
		kubeadm)
			sudo sed -i -e 's/snapshotter = "overlayfs"/snapshotter = "devmapper"/g' "${containerd_config_file}"
			sudo systemctl restart containerd ;;
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
	platform="${1:-}"
	ensure_helm
	ensure_yq

	[ "$platform" = "kcli" ] && \
	export KUBECONFIG="$HOME/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	if [ "${K8S_TEST_HOST_TYPE}" = "baremetal" ]; then
		cleanup_kata_deploy || true
	fi

	set_default_cluster_namespace

	local values_yaml
	values_yaml=$(mktemp /tmp/values_yaml.XXXXXX)

	yq -i ".k8sDistribution = \"${KUBERNETES}\""                     "${values_yaml}"
	yq -i ".image.reference = \"${DOCKER_REGISTRY}/${DOCKER_REPO}\"" "${values_yaml}"
	yq -i ".image.tag = \"${DOCKER_TAG}\""                           "${values_yaml}"
	yq -i ".env.debug = \"true\""                                    "${values_yaml}"
	yq -i ".env.shims = \"${KATA_HYPERVISOR}\""                      "${values_yaml}"
	yq -i ".env.defaultShim = \"${KATA_HYPERVISOR}\""                "${values_yaml}"
	yq -i ".env.createRuntimeClasses = \"true\""                     "${values_yaml}"
	yq -i ".env.createDefaultRuntimeClass = \"true\""                "${values_yaml}"
	yq -i ".env.allowedHypervisorAnnotations = \"default_vcpus\""    "${values_yaml}"
	yq -i ".env.snapshotterHandlerMapping = \"\""                    "${values_yaml}"
	yq -i ".env.agentHttpsProxy = \"\""                              "${values_yaml}"
	yq -i ".env.agentNoProxy = \"\""                                 "${values_yaml}"
	yq -i ".env.pullTypeMapping = \"\""                              "${values_yaml}"
	yq -i ".env.hostOS = \"\""                                       "${values_yaml}"

	if [ -n "${SNAPSHOTTER}" ]; then
		yq -i ".env.snapshotterHandlerMapping = \"${KATA_HYPERVISOR}:${SNAPSHOTTER}\"" "${values_yaml}"
	fi

	if [ "${KATA_HOST_OS}" = "cbl-mariner" ]; then
		yq -i ".env.allowedHypervisorAnnotations = \"image kernel default_vcpus\"" "${values_yaml}"
		yq -i ".env.hostOS = \"${KATA_HOST_OS}\""                                   "${values_yaml}"
	fi

	if [ "${KATA_HYPERVISOR}" = "qemu" ]; then
		yq -i ".env.allowedHypervisorAnnotations = \"image initrd kernel default_vcpus\"" "${values_yaml}"
	fi

	if [ "${KATA_HYPERVISOR}" = "qemu-tdx" ]; then
		yq -i ".env.agentHttpsProxy = \"${HTTPS_PROXY}\"" "${values_yaml}"
		yq -i ".env.agentNoProxy = \"${NO_PROXY}\""       "${values_yaml}"
	fi

	# Set the PULL_TYPE_MAPPING
	if [ "${PULL_TYPE}" != "default" ]; then
		yq -i ".env.pullTypeMapping = \"${KATA_HYPERVISOR}:${PULL_TYPE}\"" "${values_yaml}"
	fi

	echo "::group::Final kata-deploy manifests used in the test"
	cat "${values_yaml}"
	helm template "${helm_chart_dir}" --values "${values_yaml}" --namespace kube-system
	[ "$(yq .image.reference ${values_yaml})" = "${DOCKER_REGISTRY}/${DOCKER_REPO}" ] || die "Failed to set image reference"
	[ "$(yq .image.tag ${values_yaml})" = "${DOCKER_TAG}" ] || die "Failed to set image tag"
	echo "::endgroup::"

	local max_tries=3
	local interval=10
	local i=0
	# Retry loop for helm install to prevent transient failures due to instantly unreachable cluster
	set +e # Disable immediate exit on failure
	while true; do
		helm upgrade --install kata-deploy "${helm_chart_dir}" --values "${values_yaml}" --namespace kube-system --debug
		if [ $? -eq 0 ]; then
			echo "Helm install succeeded!"
			break
		fi
		i=$((i+1))
		[ $i -lt $max_tries ] && echo "Retrying after $interval seconds (Attempt $i of $(($max_tries - 1)))" || break
		sleep $interval
	done
	set -e # Re-enable immediate exit on failure
	if [ $i -eq $max_tries ]; then
		die "Failed to deploy kata-deploy after $max_tries tries"
	fi

	# `helm install --wait` does not take effect on single replicas and maxUnavailable=1 DaemonSets
	# like kata-deploy on CI. So wait for pods being Running in the "tradicional" way.
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
	kubectl_retry -n kube-system logs --tail=100 -l name=kata-deploy
	echo "::endgroup::"

	echo "::group::Runtime classes"
	kubectl_retry get runtimeclass
	echo "::endgroup::"
}

function install_kbs_client() {
	kbs_install_cli
}

function uninstall_kbs_client() {
	kbs_uninstall_cli
}

function run_tests() {
	if [ "${K8S_TEST_HOST_TYPE}" = "baremetal" ]; then
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

	[ "$platform" = "kcli" ] && \
		export KUBECONFIG="$HOME/.kcli/clusters/${CLUSTER_NAME:-kata-k8s}/auth/kubeconfig"

	if [ "${AUTO_GENERATE_POLICY}" = "yes" ] && [ "${GENPOLICY_PULL_METHOD}" = "containerd" ]; then
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
		local socket_wait_time=30
		local socket_sleep_time=3
		local cmd="sudo chmod a+rw /var/run/containerd/containerd.sock"
		waitForProcess "${socket_wait_time}" "${socket_sleep_time}" "$cmd"
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
	ensure_helm

	# Do not return after deleting only the parent object cascade=foreground
	# means also wait for child/dependent object deletion
	helm uninstall kata-deploy --ignore-not-found --wait --cascade foreground --timeout 10m --namespace kube-system --debug
}

function cleanup() {
	platform="${1:-}"
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

	# In case of canceling workflow manually, 'run_kubernetes_tests.sh' continues running and triggers new tests, 
	# resulting in the CI being in an unexpected state. So we need kill all running test scripts before cleaning up the node. 
	# See issue https://github.com/kata-containers/kata-containers/issues/9980
	delete_test_runners	|| true
	# Switch back to the default namespace and delete the tests one
	delete_test_cluster_namespace || true

	cleanup_kata_deploy
}

function deploy_snapshotter() {
	if [[ "${KATA_HYPERVISOR}" == "qemu-tdx" || "${KATA_HYPERVISOR}" == "qemu-snp" || "${KATA_HYPERVISOR}" == "qemu-sev" ]]; then
	       echo "[Skip] ${SNAPSHOTTER} is pre-installed in the TEE machine"
	       return
	fi

	echo "::group::Deploying ${SNAPSHOTTER}"
	case ${SNAPSHOTTER} in
		nydus) deploy_nydus_snapshotter ;;
		*) >&2 echo "${SNAPSHOTTER} flavour is not supported"; exit 2 ;;
	esac
	echo "::endgroup::"
}

function cleanup_snapshotter() {
	if [[ "${KATA_HYPERVISOR}" == "qemu-tdx" || "${KATA_HYPERVISOR}" == "qemu-snp" || "${KATA_HYPERVISOR}" == "qemu-sev" ]]; then
	       echo "[Skip] ${SNAPSHOTTER} is pre-installed in the TEE machine"
	       return
	fi

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
	nydus_snapshotter_url=$(get_from_kata_deps ".externals.nydus-snapshotter.url")
	nydus_snapshotter_version=$(get_from_kata_deps ".externals.nydus-snapshotter.version")
	git clone -b "${nydus_snapshotter_version}" "${nydus_snapshotter_url}" "${nydus_snapshotter_install_dir}"

	pushd "$nydus_snapshotter_install_dir"
	if [ "${K8S_TEST_HOST_TYPE}" = "baremetal" ]; then
		cleanup_nydus_snapshotter || true
	fi
	if [ "${PULL_TYPE}" == "guest-pull" ]; then
		# Enable guest pull feature in nydus snapshotter
		yq -i \
      'select(.kind == "ConfigMap").data.FS_DRIVER = "proxy"' \
      misc/snapshotter/base/nydus-snapshotter.yaml
	else
		>&2 echo "Invalid pull type"; exit 2
	fi

	# Disable to read snapshotter config from configmap
	yq -i \
    'select(.kind == "ConfigMap").data.ENABLE_CONFIG_FROM_VOLUME = "false"' \
	  misc/snapshotter/base/nydus-snapshotter.yaml
	# Enable to run snapshotter as a systemd service
	yq -i \
    'select(.kind == "ConfigMap").data.ENABLE_SYSTEMD_SERVICE = "true"' \
	  misc/snapshotter/base/nydus-snapshotter.yaml
	# Enable "runtime specific snapshotter" feature in containerd when configuring containerd for snapshotter
	yq -i \
    'select(.kind == "ConfigMap").data.ENABLE_RUNTIME_SPECIFIC_SNAPSHOTTER = "true"' \
	  misc/snapshotter/base/nydus-snapshotter.yaml

	# Pin the version of nydus-snapshotter image.
	# TODO: replace with a definitive solution (see https://github.com/kata-containers/kata-containers/issues/9742)
	yq -i \
		"select(.kind == \"DaemonSet\").spec.template.spec.containers[0].image = \"ghcr.io/containerd/nydus-snapshotter:${nydus_snapshotter_version}\"" \
		misc/snapshotter/base/nydus-snapshotter.yaml

	# Deploy nydus snapshotter as a daemonset
	kubectl_retry create -f "misc/snapshotter/nydus-snapshotter-rbac.yaml"
	if [ "${KUBERNETES}" = "k3s" ]; then
		kubectl_retry apply -k "misc/snapshotter/overlays/k3s"
	else
		kubectl_retry apply -f "misc/snapshotter/base/nydus-snapshotter.yaml"
	fi
	popd

	kubectl rollout status daemonset nydus-snapshotter -n nydus-system --timeout ${SNAPSHOTTER_DEPLOY_WAIT_TIMEOUT}

	echo "::endgroup::"
	echo "::group::nydus snapshotter logs"
	kubectl_retry logs --selector=app=nydus-snapshotter -n nydus-system
	echo "::endgroup::"
	echo "::group::nydus snapshotter describe"
	kubectl_retry describe pod --selector=app=nydus-snapshotter -n nydus-system
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
		kubectl_retry delete --ignore-not-found -k "misc/snapshotter/overlays/k3s"
	else
		kubectl_retry delete --ignore-not-found -f "misc/snapshotter/base/nydus-snapshotter.yaml"
	fi
	sleep 180s
	kubectl_retry delete --ignore-not-found -f "misc/snapshotter/nydus-snapshotter-rbac.yaml"
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
		deploy-csi-driver) return 0 ;;
		deploy-kata) deploy_kata ;;
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
		cleanup) cleanup ;;
		cleanup-kcli) cleanup "kcli" ;;
		cleanup-sev) cleanup "sev" ;;
		cleanup-snp) cleanup "snp" ;;
		cleanup-tdx) cleanup "tdx" ;;
		cleanup-garm) cleanup "garm" ;;
		cleanup-zvsi) cleanup "zvsi" ;;
		cleanup-snapshotter) cleanup_snapshotter ;;
		delete-csi-driver) return 0 ;;
		delete-coco-kbs) delete_coco_kbs ;;
		delete-cluster) cleanup "aks" ;;
		delete-cluster-kcli) delete_cluster_kcli ;;
		uninstall-kbs-client) uninstall_kbs_client ;;
		*) >&2 echo "Invalid argument"; exit 2 ;;
	esac
}

main "$@"
