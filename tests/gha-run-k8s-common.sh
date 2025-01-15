#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

tests_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${tests_dir}/common.bash"
kubernetes_dir="${tests_dir}/integration/kubernetes"
helm_chart_dir="${repo_root_dir}/tools/packaging/kata-deploy/helm-chart/kata-deploy"

AZ_REGION="${AZ_REGION:-eastus}"
AZ_NODEPOOL_TAGS="${AZ_NODEPOOL_TAGS:-}"
GENPOLICY_PULL_METHOD="${GENPOLICY_PULL_METHOD:-oci-distribution}"
GH_PR_NUMBER="${GH_PR_NUMBER:-}"
HELM_DEFAULT_INSTALLATION="${HELM_DEFAULT_INSTALLATION:-false}"
HELM_AGENT_HTTPS_PROXY="${HELM_AGENT_HTTPS_PROXY:-}"
HELM_AGENT_NO_PROXY="${HELM_AGENT_NO_PROXY:-}"
HELM_ALLOWED_HYPERVISOR_ANNOTATIONS="${HELM_ALLOWED_HYPERVISOR_ANNOTATIONS:-}"
HELM_CREATE_RUNTIME_CLASSES="${HELM_CREATE_RUNTIME_CLASSES:-}"
HELM_CREATE_DEFAULT_RUNTIME_CLASS="${HELM_CREATE_DEFAULT_RUNTIME_CLASS:-}"
HELM_DEBUG="${HELM_DEBUG:-}"
HELM_DEFAULT_SHIM="${HELM_DEFAULT_SHIM:-}"
HELM_HOST_OS="${HELM_HOST_OS:-}"
HELM_IMAGE_REFERENCE="${HELM_IMAGE_REFERENCE:-}"
HELM_IMAGE_TAG="${HELM_IMAGE_TAG:-}"
HELM_K8S_DISTRIBUTION="${HELM_K8S_DISTRIBUTION:-}"
HELM_PULL_TYPE_MAPPING="${HELM_PULL_TYPE_MAPPING:-}"
HELM_SHIMS="${HELM_SHIMS:-}"
HELM_SNAPSHOTTER_HANDLER_MAPPING="${HELM_SNAPSHOTTER_HANDLER_MAPPING:-}"
HELM_EXPERIMENTAL_SETUP_SNAPSHOTTER="${HELM_EXPERIMENTAL_SETUP_SNAPSHOTTER:-}"
HELM_EXPERIMENTAL_FORCE_GUEST_PULL="${HELM_EXPERIMENTAL_FORCE_GUEST_PULL:-}"
KATA_DEPLOY_WAIT_TIMEOUT="${KATA_DEPLOY_WAIT_TIMEOUT:-600}"
KATA_HOST_OS="${KATA_HOST_OS:-}"
KUBERNETES="${KUBERNETES:-}"
K8S_TEST_HOST_TYPE="${K8S_TEST_HOST_TYPE:-small}"
TEST_CLUSTER_NAMESPACE="${TEST_CLUSTER_NAMESPACE:-}"
CONTAINER_RUNTIME="${CONTAINER_RUNTIME:-containerd}"
SNAPSHOTTER="${SNAPSHOTTER:-}"

function _print_instance_type() {
	case "${K8S_TEST_HOST_TYPE}" in
		small)
			echo "Standard_D2s_v5"
			;;
		all|normal)
			echo "Standard_D4s_v5"
			;;
		*)
			echo "Unknown instance type '${K8S_TEST_HOST_TYPE}'" >&2
			exit 1
	esac
}

# Print the cluster name set by $AKS_NAME or generated out of runtime
# metadata (e.g. pull request number, commit SHA, etc).
#
function _print_cluster_name() {
	local test_type="${1:-k8s}"
	local short_sha
	local metadata

	if [[ -n "${AKS_NAME:-}" ]]; then
		echo "${AKS_NAME}"
	else
		short_sha="$(git rev-parse --short=12 HEAD)"
		metadata="${GH_PR_NUMBER}-${short_sha}-${KATA_HYPERVISOR}-${KATA_HOST_OS}-${SNAPSHOTTER}-amd64-${K8S_TEST_HOST_TYPE:0:1}-${GENPOLICY_PULL_METHOD:0:1}"
		# Compute the SHA1 digest of the metadata part to keep the name less
		# than the limit of 63 chars of AKS
		echo "${test_type}-$(sha1sum <<< "${metadata}" | cut -d' ' -f1)"
	fi
}

function _print_rg_name() {
	test_type="${1:-k8s}"

	echo "${AZ_RG:-"kataCI-$(_print_cluster_name "${test_type}")"}"
}

# Enable the approuting routing add-on to AKS.
# Use with ingress to expose a service API externally.
#
function enable_cluster_approuting() {
	local test_type="${1:-k8s}"
	local cluster_name
	local rg

	rg="$(_print_rg_name "${test_type}")"
	cluster_name="$(_print_cluster_name "${test_type}")"

	az aks approuting enable -g "${rg}" -n "${cluster_name}"
}

function create_cluster() {
	test_type="${1:-k8s}"
	local short_sha
	local tags
	local rg

	# First ensure it didn't fail to get cleaned up from a previous run.
	delete_cluster "${test_type}" || true

	rg="$(_print_rg_name "${test_type}")"

	short_sha="$(git rev-parse --short=12 HEAD)"
	tags=("GH_PR_NUMBER=${GH_PR_NUMBER:-}" \
		"SHORT_SHA=${short_sha}" \
		"KATA_HYPERVISOR=${KATA_HYPERVISOR}"\
		"KATA_HOST_OS=${KATA_HOST_OS:-}" \
		"SNAPSHOTTER=${SNAPSHOTTER}" \
		"K8S_TEST_HOST_TYPE=${K8S_TEST_HOST_TYPE:0:1}" \
		"GENPOLICY_PULL_METHOD=${GENPOLICY_PULL_METHOD:0:1}")

	az group create \
		-l "${AZ_REGION}" \
		-n "${rg}"

	# Required by e.g. AKS App Routing for KBS installation.
	az extension add --name aks-preview

	# Adding a double quote on the last line ends up causing issues
	# ine the cbl-mariner installation.  Because of that, let's just
	# disable the warning for this specific case.
	# shellcheck disable=SC2046
	az aks create \
		-g "${rg}" \
		--node-resource-group "node-${rg}" \
		-n "$(_print_cluster_name "${test_type}")" \
		-s "$(_print_instance_type)" \
		--node-count 1 \
		--generate-ssh-keys \
		--tags "${tags[@]}" \
		$([[ "${KATA_HOST_OS}" = "cbl-mariner" ]] && echo "--os-sku AzureLinux --workload-runtime KataMshvVmIsolation") \
		$([[ -n "${AZ_NODEPOOL_TAGS}" ]] && echo "--nodepool-tags ${AZ_NODEPOOL_TAGS}")
}

function install_bats() {
	source /etc/os-release
	case "${ID}" in
		ubuntu)
			# Installing bats from the noble repo.
			sudo apt install -y software-properties-common
			sudo add-apt-repository 'deb http://archive.ubuntu.com/ubuntu/ noble universe'
			sudo apt install -y bats
			sudo add-apt-repository --remove 'deb http://archive.ubuntu.com/ubuntu/ noble universe'
			;;
		*)
			echo "${ID} is not a supported distro, install bats manually"
			;;
	esac

}

# Install the kustomize tool in /usr/local/bin if it doesn't exist on
# the system yet.
#
function install_kustomize() {
	local arch
	local checksum
	local version

	if command -v kustomize >/dev/null; then
		return
	fi

	ensure_yq
	version=$(get_from_kata_deps ".externals.kustomize.version")
	arch=$(arch_to_golang)
	checksum=$(get_from_kata_deps ".externals.kustomize.checksum.${arch}")

	local tarball="kustomize_${version}_linux_${arch}.tar.gz"
	curl -Lf -o "${tarball}" "https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize/${version}/${tarball}"

	local rc=0
	echo "${checksum} ${tarball}" | sha256sum -c || rc=$?
	[[ ${rc} -eq 0 ]] && sudo tar -xvzf "${tarball}" -C /usr/local/bin || rc=$?
	rm -f "${tarball}"
	[[ ${rc} -eq 0 ]]
}

function get_cluster_credentials() {
	test_type="${1:-k8s}"

	az aks get-credentials \
		--overwrite-existing \
		-g "$(_print_rg_name "${test_type}")" \
		-n "$(_print_cluster_name "${test_type}")"
}

function delete_cluster() {
	test_type="${1:-k8s}"
	local rg
	rg="$(_print_rg_name "${test_type}")"

	if [[ "$(az group exists -g "${rg}")" == "true" ]]; then
		az group delete -g "${rg}" --yes
	fi
}

function delete_cluster_kcli() {
	CLUSTER_NAME="${CLUSTER_NAME:-kata-k8s}"
	kcli delete -y kube "${CLUSTER_NAME}"
}

function get_nodes_and_pods_info() {
	kubectl debug "$(kubectl get nodes -o name)" -it --image=quay.io/kata-containers/kata-debug:latest || true
	kubectl get pods -o name | grep node-debugger | xargs kubectl delete || true
}

function deploy_k0s() {
	if [[ "${CONTAINER_RUNTIME}" == "crio" ]]; then
		url=$(get_from_kata_deps ".externals.k0s.url")

		k0s_version_param=""
		version=$(get_from_kata_deps ".externals.k0s.version")
		if [[ -n "${version}" ]]; then
			k0s_version_param="K0S_VERSION=${version}"
		fi

		curl -sSLf "${url}" | sudo "${k0s_version_param}" sh
	else
		curl -sSLf -sSLf https://get.k0s.sh | sudo sh
	fi

	# In this case we explicitly want word splitting when calling k0s
	# with extra parameters.
	# shellcheck disable=SC2086
	sudo k0s install controller --single ${KUBERNETES_EXTRA_PARAMS:-}

	# kube-router decided to use :8080 for its metrics, and this seems
	# to be a change that affected k0s 1.30.0+, leading to kube-router
	# pod crashing all the time and anything can actually be started
	# after that.
	#
	# Due to this issue, let's simply use a different port (:9999) and
	# move on with our tests.
	sudo mkdir -p /etc/k0s
	k0s config create | sudo tee /etc/k0s/k0s.yaml
	sudo sed -i -e "s/metricsPort: 8080/metricsPort: 9999/g" /etc/k0s/k0s.yaml

	sudo k0s start

	# This is an arbitrary value that came up from local tests
	sleep 120s

	# Download the kubectl binary into /usr/bin so we can avoid depending
	# on `k0s kubectl` command
	ARCH=$(arch_to_golang)

	kubectl_version=$(sudo k0s kubectl version 2>/dev/null | grep "Client Version" | sed -e 's/Client Version: //')
	sudo curl -fL --progress-bar -o /usr/bin/kubectl https://dl.k8s.io/release/"${kubectl_version}"/bin/linux/"${ARCH}"/kubectl
	sudo chmod +x /usr/bin/kubectl

	mkdir -p ~/.kube
	sudo cp /var/lib/k0s/pki/admin.conf ~/.kube/config
	sudo chown "${USER}":"${USER}" ~/.kube/config
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
	ARCH=$(arch_to_golang)

	kubectl_version=$(/usr/local/bin/k3s kubectl version --client=true 2>/dev/null | grep "Client Version" | sed -e 's/Client Version: //' -e 's/+k3s[0-9]\+//')
	sudo curl -fL --progress-bar -o /usr/bin/kubectl https://dl.k8s.io/release/"${kubectl_version}"/bin/linux/"${ARCH}"/kubectl
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
		-P image="${CLUSTER_IMAGE:-ubuntu2204}" \
		-P sdn=flannel \
		-P nfs=false \
		-P disk_size="${CLUSTER_DISK_SIZE:-20}" \
		"${CLUSTER_NAME}"

	export KUBECONFIG="${HOME}/.kcli/clusters/${CLUSTER_NAME}/auth/kubeconfig"

	local cmd="kubectl get nodes | grep '.*worker.*\<Ready\>'"
	echo "Wait at least one worker be Ready"
	if ! waitForProcess "330" "30" "${cmd}"; then
		echo "ERROR: worker nodes not ready."
		kubectl get nodes
		return 1
	fi

	# Ensure that system pods are running or completed.
	cmd="[ \$(kubectl get pods -A --no-headers | grep -v 'Running\|Completed' | wc -l) -eq 0 ]"
	echo "Wait system pods be running or completed"
	if ! waitForProcess "90" "30" "${cmd}"; then
		echo "ERROR: not all pods are Running or Completed."
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
	sudo chown "${USER}":"${USER}" ~/.kube/config
}

function deploy_microk8s() {
	sudo snap install microk8s --classic
	sudo usermod -a -G microk8s "${USER}"
	mkdir -p ~/.kube
	# As we want to call microk8s with sudo, we're safe to ignore SC2024 here
	# shellcheck disable=SC2024
	sudo microk8s kubectl config view --raw > ~/.kube/config
	sudo chown "${USER}":"${USER}" ~/.kube/config

	# These are arbitrary values
	sudo microk8s status --wait-ready --timeout 300

	# install kubectl
	ARCH=$(arch_to_golang)
	kubectl_version=$(sudo microk8s version | grep -oe 'v[0-9]\+\(\.[0-9]\+\)*')
	sudo curl -fL --progress-bar -o /usr/bin/kubectl https://dl.k8s.io/release/"${kubectl_version}"/bin/linux/"${ARCH}"/kubectl
	sudo chmod +x /usr/bin/kubectl
	sudo rm -rf /usr/local/bin/kubectl
}

function install_system_dependencies() {
	dependencies="${1}"

	sudo apt-get update
	sudo apt-get -y install "${dependencies}"
}

function load_k8s_needed_modules() {
	sudo modprobe overlay
	sudo modprobe br_netfilter
}

function set_k8s_network_parameters() {
	sudo sysctl -w net.bridge.bridge-nf-call-iptables=1
	sudo sysctl -w net.ipv4.ip_forward=1
	sudo sysctl -w net.bridge.bridge-nf-call-ip6tables=1
}

function disable_swap() {
	sudo swapoff -a
}

# Always deploys the latest k8s version
function do_deploy_k8s() {
	# Add the pkgs.k8s.io repo
	curl -fsSL https://pkgs.k8s.io/core:/stable:/$(curl -Ls https://dl.k8s.io/release/stable.txt | cut -d. -f-2)/deb/Release.key | sudo gpg --batch --yes --no-tty --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg
	echo "deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] https://pkgs.k8s.io/core:/stable:/$(curl -Ls https://dl.k8s.io/release/stable.txt | cut -d. -f-2)/deb/ /" | sudo tee /etc/apt/sources.list.d/kubernetes.list

	# Pin the packages to ensure they'll be downloaded from the pkgs.k8s.io repo
	#
	# This is needed as the github runner uses the azure repo which already has
	# kubernetes packages, and those packages simply don't work well with the
	# runner.
	cat <<EOF | sudo tee /etc/apt/preferences.d/kubernetes
Package: kubelet kubeadm kubectl cri-tools kubernetes-cni
Pin: origin pkgs.k8s.io
Pin-Priority: 1000
EOF

	# Install the packages
	sudo apt-get update
   	sudo apt-get -y install kubeadm kubelet kubectl --allow-downgrades
   	sudo apt-mark hold kubeadm kubelet kubectl

	# Deploy k8s using kubeadm
   	sudo kubeadm init --pod-network-cidr=10.244.0.0/16
	mkdir -p $HOME/.kube
	sudo cp -i /etc/kubernetes/admin.conf $HOME/.kube/config
	sudo chown $(id -u):$(id -g) $HOME/.kube/config

	# Deploy flannel
	kubectl apply -f https://github.com/flannel-io/flannel/releases/latest/download/kube-flannel.yml

	# Untaint the node
	kubectl taint nodes --all node-role.kubernetes.io/control-plane-
}

# container_engine: containerd (only containerd is supported for now, support for crio is welcome)
# container_engine_version: major.minor (and then we'll install the latest patch release matching that major.minor)
function deploy_vanilla_k8s() {
	container_engine="${1}"
	container_engine_version="${2}"

	[[ -z "${container_engine}" ]] && die "container_engine is required"
	[[ -z "${container_engine_version}" ]] && die "container_engine_version is required"

	install_system_dependencies "runc"
	load_k8s_needed_modules
	set_k8s_network_parameters
	disable_swap
	case "${container_engine}" in
		containerd)
			install_cri_containerd "${container_engine_version}"
			sudo mkdir -p /etc/containerd
			containerd config default | sed -e 's/SystemdCgroup = false/SystemdCgroup = true/' | sudo tee /etc/containerd/config.toml
			;;
		*) die "${container_engine} is not a container engine supported by this script" ;;
	esac
	sudo systemctl daemon-reload && sudo systemctl restart "${container_engine}"
	do_deploy_k8s
}

function deploy_k8s() {
	echo "::group::Deploying ${KUBERNETES}"

	case "${KUBERNETES}" in
		k0s) deploy_k0s ;;
		k3s) deploy_k3s ;;
		rke2) deploy_rke2 ;;
		microk8s) deploy_microk8s ;;
		vanilla)
			if [[ "${SNAPSHOTTER:-}" == "erofs" ]]; then
				# Install erofs specific dependencies
				sudo apt-get update
				sudo apt-get -y install erofs-utils fsverity

				# Load the erofs module
				sudo modprobe erofs

				# Ensure fsverity is enabled on the disk, otherwise
				# fsverity won't work on the erofs-snapshotter side.
				#
				# Get the root device to enable fsverity on the disk.
				root_device="$(findmnt -v -n -o SOURCE /)"
				# This command is not destructive, at all, and that's
				# the way we should enable verity support on a live disk.
				sudo tune2fs -O verity "${root_device}"
			fi
			deploy_vanilla_k8s ${CONTAINER_ENGINE} ${CONTAINER_ENGINE_VERSION}
			;;
		*) >&2 echo "${KUBERNETES} flavour is not supported"; exit 2 ;;
	esac

	echo "::endgroup::"
}

function set_test_cluster_namespace() {
	# Delete any spurious tests namespace that was left behind
	kubectl delete namespace "${TEST_CLUSTER_NAMESPACE}" &> /dev/null || true

	# Create a new namespace for the tests and switch to it
	kubectl apply -f "${kubernetes_dir}/runtimeclass_workloads/tests-namespace.yaml"
	kubectl config set-context --current --namespace="${TEST_CLUSTER_NAMESPACE}"
}

function set_default_cluster_namespace() {
	kubectl config set-context --current --namespace=default
}

function delete_test_cluster_namespace() {
	kubectl delete namespace "${TEST_CLUSTER_NAMESPACE}"
	set_default_cluster_namespace
}

function delete_test_runners(){
	echo "Delete test scripts"
	local scripts_names=( "run_kubernetes_tests.sh" "bats" )
	for script_name in "${scripts_names[@]}"; do
		pids=$(pgrep -f "${script_name}")
		if [[ -n "${pids}" ]]; then
			echo "${pids}" | xargs sudo kill -SIGTERM >/dev/null 2>&1 || true
		fi
	done
}

function helm_helper() {
	local max_tries
	local interval
	local i
	local values_yaml

	ensure_yq
	ensure_helm

	# Update dependencies before configuring values
	pushd ${helm_chart_dir}
	helm dependencies update
	popd

	# Create temporary values file for customization
	# Start with values.yaml which has all shims enabled by default
	# Use example files only for specific hypervisor types that need different configurations
	values_yaml=$(mktemp -t values_yaml.XXXXXX)

	# Determine which values file to use as base
	local base_values_file="${helm_chart_dir}/values.yaml"
	if [[ -n "${KATA_HYPERVISOR}" ]]; then
		case "${KATA_HYPERVISOR}" in
			*nvidia-gpu*)
				# Use NVIDIA GPU example file
				if [[ -f "${helm_chart_dir}/try-kata-nvidia-gpu.values.yaml" ]]; then
					base_values_file="${helm_chart_dir}/try-kata-nvidia-gpu.values.yaml"
				fi
				;;
			qemu-snp|qemu-tdx|qemu-se|qemu-se-runtime-rs|qemu-cca|qemu-coco-dev|qemu-coco-dev-runtime-rs)
				# Use TEE example file
				if [[ -f "${helm_chart_dir}/try-kata-tee.values.yaml" ]]; then
					base_values_file="${helm_chart_dir}/try-kata-tee.values.yaml"
				fi
				;;
		esac
	fi

	# Copy the base values file to the temporary file
	cp "${base_values_file}" "${values_yaml}"

	# Enable node-feature-discovery deployment
	yq -i ".node-feature-discovery.enabled = true" "${values_yaml}"

	# Do not enable on cbl-mariner yet, as the deployment is failing on those
	if [[ "${HELM_HOST_OS}" == "cbl-mariner" ]]; then
		yq -i ".node-feature-discovery.enabled = false" "${values_yaml}"
	fi

	# Do not enable on nvidia-gpu-* tests, as it'll be deployed by the GPU operator
	if [[ "${KATA_HYPERVISOR}" == *"nvidia-gpu"* ]]; then
		yq -i ".node-feature-discovery.enabled = false" "${values_yaml}"
		yq -i ".runtimeClasses.createDefault = true" "${values_yaml}"
	fi

	if [[ -z "${HELM_IMAGE_REFERENCE}" ]]; then
		die "HELM_IMAGE_REFERENCE environment variable cannot be empty."
	fi
	yq -i ".image.reference = \"${HELM_IMAGE_REFERENCE}\"" "${values_yaml}"

	if [[ -z "${HELM_IMAGE_TAG}" ]]; then
		die "HELM_IMAGE_TAG environment variable cannot be empty."
	fi
	yq -i ".image.tag = \"${HELM_IMAGE_TAG}\"" "${values_yaml}"

	[[ -n "${HELM_K8S_DISTRIBUTION}" ]] && yq -i ".k8sDistribution = \"${HELM_K8S_DISTRIBUTION}\"" "${values_yaml}"

	if [[ "${HELM_DEFAULT_INSTALLATION}" = "false" ]]; then
		# Disable all shims first (in case we started from an example file with shims enabled)
		# Then we'll enable only the ones specified in HELM_SHIMS
		for shim_key in $(yq '.shims | keys | .[]' "${values_yaml}" 2>/dev/null); do
			yq -i ".shims.${shim_key}.enabled = false" "${values_yaml}"
		done

		# Use new structured format
		if [[ -n "${HELM_DEBUG}" ]]; then
			if [[ "${HELM_DEBUG}" == "true" ]]; then
				yq -i ".debug = true" "${values_yaml}"
			else
				yq -i ".debug = false" "${values_yaml}"
			fi
		fi

		# Configure shims using new structured format
		if [[ -n "${HELM_SHIMS}" ]]; then
			# HELM_SHIMS is a space-separated list of shim names
			# Enable each shim and set supported architectures
			# TEE shims that need defaults unset (will be set based on env vars)
			tee_shims="qemu-se qemu-se-runtime-rs qemu-cca qemu-snp qemu-tdx qemu-coco-dev qemu-coco-dev-runtime-rs qemu-nvidia-gpu-snp qemu-nvidia-gpu-tdx"

			for shim in ${HELM_SHIMS}; do
				# Determine supported architectures based on shim name
				# Most shims support amd64 and arm64, some have specific arch requirements
				case "${shim}" in
					qemu-se|qemu-se-runtime-rs)
						yq -i ".shims.${shim}.enabled = true" "${values_yaml}"
						yq -i ".shims.${shim}.supportedArches = [\"s390x\"]" "${values_yaml}"
						;;
					qemu-cca)
						yq -i ".shims.${shim}.enabled = true" "${values_yaml}"
						yq -i ".shims.${shim}.supportedArches = [\"arm64\"]" "${values_yaml}"
						;;
					qemu-snp|qemu-tdx|qemu-nvidia-gpu-snp|qemu-nvidia-gpu-tdx)
						yq -i ".shims.${shim}.enabled = true" "${values_yaml}"
						yq -i ".shims.${shim}.supportedArches = [\"amd64\"]" "${values_yaml}"
						;;
					qemu-runtime-rs|qemu-coco-dev|qemu-coco-dev-runtime-rs)
						yq -i ".shims.${shim}.enabled = true" "${values_yaml}"
						yq -i ".shims.${shim}.supportedArches = [\"amd64\", \"s390x\"]" "${values_yaml}"
						;;
					qemu-nvidia-gpu)
						yq -i ".shims.${shim}.enabled = true" "${values_yaml}"
						yq -i ".shims.${shim}.supportedArches = [\"amd64\", \"arm64\"]" "${values_yaml}"
						;;
					*)
						# Default: support amd64, arm64, s390x, ppc64le
						yq -i ".shims.${shim}.enabled = true" "${values_yaml}"
						yq -i ".shims.${shim}.supportedArches = [\"amd64\", \"arm64\", \"s390x\", \"ppc64le\"]" "${values_yaml}"
						;;
				esac

				# Explicitly unset defaults for TEE shims - these will be set based on env vars:
				# - snapshotter: nydus if HELM_SNAPSHOTTER_HANDLER_MAPPING is set
				# - forceGuestPull: true if HELM_EXPERIMENTAL_FORCE_GUEST_PULL is set
				# - guestPull: true if HELM_PULL_TYPE_MAPPING contains guest-pull
				if echo "${tee_shims}" | grep -qw "${shim}"; then
					yq -i ".shims.${shim}.containerd.snapshotter = \"\"" "${values_yaml}"
					yq -i ".shims.${shim}.containerd.forceGuestPull = false" "${values_yaml}"
					yq -i ".shims.${shim}.crio.guestPull = false" "${values_yaml}"
				fi
			done
		fi

		# Set default shim per architecture
		if [[ -n "${HELM_DEFAULT_SHIM}" ]]; then
			# Set for all architectures (can be overridden per-arch if needed)
			yq -i ".defaultShim.amd64 = \"${HELM_DEFAULT_SHIM}\"" "${values_yaml}"
			yq -i ".defaultShim.arm64 = \"${HELM_DEFAULT_SHIM}\"" "${values_yaml}"
			yq -i ".defaultShim.s390x = \"${HELM_DEFAULT_SHIM}\"" "${values_yaml}"
			yq -i ".defaultShim.ppc64le = \"${HELM_DEFAULT_SHIM}\"" "${values_yaml}"
		fi

		# Configure snapshotter setup using new structured format
		# Note: snapshotter.setup (global) is separate from containerd.snapshotter (per-shim)
		# Always unset first to clear any defaults from base file
		yq -i ".snapshotter.setup = []" "${values_yaml}"

		# For TDX and SNP shims, snapshotter.setup must ALWAYS be disabled in CI
		# Check if any TDX/SNP shims are enabled
		disable_snapshotter_setup=false
		for shim in ${HELM_SHIMS}; do
			case "${shim}" in
				qemu-tdx|qemu-snp|qemu-nvidia-gpu-tdx|qemu-nvidia-gpu-snp)
					disable_snapshotter_setup=true
					break
					;;
			esac
		done

		# Safety check: Fail if EXPERIMENTAL_SETUP_SNAPSHOTTER is set when using SNP/TDX shims
		if [[ "${disable_snapshotter_setup}" == "true" ]] && [[ -n "${HELM_EXPERIMENTAL_SETUP_SNAPSHOTTER}" ]]; then
			die "ERROR: HELM_EXPERIMENTAL_SETUP_SNAPSHOTTER cannot be set when using SNP/TDX shims (qemu-snp, qemu-tdx, qemu-nvidia-gpu-snp, qemu-nvidia-gpu-tdx). snapshotter.setup must always be disabled for these shims."
		fi

		if [[ -n "${HELM_EXPERIMENTAL_SETUP_SNAPSHOTTER}" ]]; then
			# Convert space-separated or comma-separated list to YAML array
			IFS=', ' read -ra snapshotter_list <<< "${HELM_EXPERIMENTAL_SETUP_SNAPSHOTTER}"
			for snapshotter in "${snapshotter_list[@]}"; do
				yq -i ".snapshotter.setup += [\"${snapshotter}\"]" "${values_yaml}"
			done
		fi

		if [[ -z "${HELM_SHIMS}" ]]; then
			die "A list of shims is expected but none was provided"
		fi

		# Convert simple format to per-shim format for all enabled shims
		# HELM_ALLOWED_HYPERVISOR_ANNOTATIONS: if not in per-shim format (no colon), convert to per-shim format
		# Output format: "qemu:foo,bar clh:foo" (space-separated entries, each with shim:annotations where annotations are comma-separated)
		# Example: "foo bar" with shim "qemu-tdx" -> "qemu-tdx:foo,bar"
		if [[ "${HELM_ALLOWED_HYPERVISOR_ANNOTATIONS}" != *:* ]]; then
			# Simple format: convert to per-shim format for all enabled shims
			# "default_vcpus" -> "qemu-tdx:default_vcpus" (single shim)
			# "image kernel default_vcpus" -> "qemu-tdx:image,kernel,default_vcpus" (single shim)
			# "default_vcpus" -> "qemu-tdx:default_vcpus qemu-snp:default_vcpus" (multiple shims)
			local converted_annotations=""
			for shim in ${HELM_SHIMS}; do
				if [[ -n "${converted_annotations}" ]]; then
					converted_annotations+=" "
				fi
				# Convert space-separated to comma-separated: "foo bar" -> "foo,bar"
				local annotations_comma=$(echo "${HELM_ALLOWED_HYPERVISOR_ANNOTATIONS}" | sed 's/ /,/g')
				converted_annotations+="${shim}:${annotations_comma}"
			done
			HELM_ALLOWED_HYPERVISOR_ANNOTATIONS="${converted_annotations}"
		fi

		# HELM_AGENT_HTTPS_PROXY: if not in per-shim format (no equals), convert to per-shim format
		if [[ "${HELM_AGENT_HTTPS_PROXY}" != *=* ]]; then
			# Simple format: convert to per-shim format for all enabled shims
			# "http://proxy:8080" -> "qemu-tdx=http://proxy:8080;qemu-snp=http://proxy:8080"
			local converted_proxy=""
			for shim in ${HELM_SHIMS}; do
				if [[ -n "${converted_proxy}" ]]; then
					converted_proxy+=";"
				fi
				converted_proxy+="${shim}=${HELM_AGENT_HTTPS_PROXY}"
			done
			HELM_AGENT_HTTPS_PROXY="${converted_proxy}"
		fi

		# HELM_AGENT_NO_PROXY: if not in per-shim format (no equals), convert to per-shim format
		if [[ "${HELM_AGENT_NO_PROXY}" != *=* ]]; then
			# Simple format: convert to per-shim format for all enabled shims
			# "localhost,127.0.0.1" -> "qemu-tdx=localhost,127.0.0.1;qemu-snp=localhost,127.0.0.1"
			local converted_noproxy=""
			for shim in ${HELM_SHIMS}; do
				if [[ -n "${converted_noproxy}" ]]; then
					converted_noproxy+=";"
				fi
				converted_noproxy+="${shim}=${HELM_AGENT_NO_PROXY}"
			done
			HELM_AGENT_NO_PROXY="${converted_noproxy}"
		fi

		# Set allowed hypervisor annotations (now in per-shim format)
		# Format: "qemu-tdx:default_vcpus qemu-snp:default_vcpus" (space-separated, colon-separated shim:annotations)
		if [[ -n "${HELM_ALLOWED_HYPERVISOR_ANNOTATIONS}" ]]; then
			# Parse space-separated annotations and set values for matching shims
			IFS=' ' read -ra annotations <<< "${HELM_ALLOWED_HYPERVISOR_ANNOTATIONS}"
			for m in "${annotations[@]}"; do
				# Check if this mapping has a colon (shim-specific)
				if [[ "${m}" == *:* ]]; then
					# Shim-specific mapping like "qemu:foo,bar"
					local shim="${m%:*}"
					local value="${m#*:}"

					# Convert comma-separated list to YAML array
					IFS=',' read -ra final_annotations <<< "${value}"
					yq -i ".shims.${shim}.allowedHypervisorAnnotations = []" "${values_yaml}"
					for annotation in "${final_annotations[@]}"; do
						# Trim whitespace
						annotation=$(echo "${annotation}" | sed 's/^[[:space:]]//;s/[[:space:]]$//')
						if [[ -n "${annotation}" ]]; then
							yq -i ".shims.${shim}.allowedHypervisorAnnotations += [\"${annotation}\"]" "${values_yaml}"
						fi
					done
				fi
			done
		fi

		# Set agent proxy settings (now in per-shim format)
		# Format: "qemu-tdx=http://proxy:8080;qemu-snp=http://proxy:8080" (semicolon-separated "shim=proxy" mappings)
		if [[ -n "${HELM_AGENT_HTTPS_PROXY}" ]]; then
			# Parse semicolon-separated "shim=proxy" mappings and set values for matching shims
			IFS=';' read -ra proxy_mappings <<< "${HELM_AGENT_HTTPS_PROXY}"
			for mapping in "${proxy_mappings[@]}"; do
				local shim="${mapping%%=*}"
				local value="${mapping#*=}"
				yq -i ".shims.${shim}.agent.httpsProxy = \"${value}\"" "${values_yaml}"
			done
		fi

		if [[ -n "${HELM_AGENT_NO_PROXY}" ]]; then
			# Parse semicolon-separated "shim=no_proxy" mappings and set values for matching shims
			IFS=';' read -ra noproxy_mappings <<< "${HELM_AGENT_NO_PROXY}"
			for mapping in "${noproxy_mappings[@]}"; do
				local shim="${mapping%%=*}"
				local value="${mapping#*=}"
				yq -i ".shims.${shim}.agent.noProxy = \"${value}\"" "${values_yaml}"
			done
		fi

		# Set snapshotter handler mapping (format: "shim:snapshotter")
		if [[ -n "${HELM_SNAPSHOTTER_HANDLER_MAPPING}" ]]; then
			# Parse format "shim:snapshotter" or "shim1:snapshotter1,shim2:snapshotter2"
			IFS=',' read -ra mappings <<< "${HELM_SNAPSHOTTER_HANDLER_MAPPING}"
			for mapping in "${mappings[@]}"; do
				shim="${mapping%%:*}"
				snapshotter="${mapping##*:}"
				yq -i ".shims.${shim}.containerd.snapshotter = \"${snapshotter}\"" "${values_yaml}"
				# When using a snapshotter (like nydus), ensure forceGuestPull is false
				# to prevent EXPERIMENTAL_FORCE_GUEST_PULL from being incorrectly set
				yq -i ".shims.${shim}.containerd.forceGuestPull = false" "${values_yaml}"
			done
		fi

		# Set pull type mapping (format: "shim:pullType")
		if [[ -n "${HELM_PULL_TYPE_MAPPING}" ]]; then
			# Parse format "shim:pullType" or "shim1:pullType1,shim2:pullType2"
			IFS=',' read -ra mappings <<< "${HELM_PULL_TYPE_MAPPING}"
			for mapping in "${mappings[@]}"; do
				shim="${mapping%%:*}"
				pull_type="${mapping##*:}"
				if [[ "${pull_type}" == "guest-pull" ]]; then
					# PULL_TYPE_MAPPING with guest-pull only sets crio.guestPull.
					yq -i ".shims.${shim}.crio.guestPull = true" "${values_yaml}"
				else
					echo "WARN: Unsupported pull type '${pull_type}' for shim '${shim}' in HELM_PULL_TYPE_MAPPING."
				fi
			done
		fi

		# Set experimental force guest pull (if set to shim name, enable it)
		if [[ "${HELM_EXPERIMENTAL_FORCE_GUEST_PULL}" ]]; then
			# Parse format "shim1,shim2,..."
			IFS=',' read -ra shims <<< "${HELM_EXPERIMENTAL_FORCE_GUEST_PULL}"
			for shim in "${shims[@]}"; do
				yq -i ".shims.${shim}.containerd.forceGuestPull = true" "${values_yaml}"
				# When using EXPERIMENTAL_FORCE_GUEST_PULL, ensure a snapshotter (like nydus) is not set,
				# to prevent the snapshotter from being incorrectly set
				yq -i ".shims.${shim}.containerd.snapshotter = \"\"" "${values_yaml}"
			done
		fi

		[[ -n "${HELM_CREATE_RUNTIME_CLASSES}" ]] && yq -i ".runtimeClasses.enabled = ${HELM_CREATE_RUNTIME_CLASSES}" "${values_yaml}"
		[[ -n "${HELM_CREATE_DEFAULT_RUNTIME_CLASS}" ]] && yq -i ".runtimeClasses.createDefault = ${HELM_CREATE_DEFAULT_RUNTIME_CLASS}" "${values_yaml}"

		# Legacy env.* settings that don't have structured equivalents yet
		[[ -n "${HELM_HOST_OS}" ]] && yq -i ".env.hostOS=\"${HELM_HOST_OS}\"" "${values_yaml}"
	fi

	echo "::group::Final kata-deploy manifests used in the test"
	cat "${values_yaml}"
	echo ""
	helm template "${helm_chart_dir}" --values "${values_yaml}" --namespace kube-system
	[[ "$(yq .image.reference "${values_yaml}")" = "${HELM_IMAGE_REFERENCE}" ]] || die "Failed to set image reference"
	[[ "$(yq .image.tag "${values_yaml}")" = "${HELM_IMAGE_TAG}" ]] || die "Failed to set image tag"
	echo "::endgroup::"

	# Ensure any potential leftover is cleaned up ... and this secret usually is not in case of previous failures
	kubectl delete secret sh.helm.release.v1.kata-deploy.v1 -n kube-system || true

	max_tries=3
	interval=10
	i=10

	# Retry loop for helm install to prevent transient failures due to instantly unreachable cluster
	set +e # Disable immediate exit on failure
	while true; do
		helm upgrade --install kata-deploy "${helm_chart_dir}" --values "${values_yaml}" --namespace kube-system --debug
		ret=${?}
		if [[ ${ret} -eq 0 ]]; then
			echo "Helm install succeeded!"
			break
		fi
		i=$((i+1))
		if [[ ${i} -lt ${max_tries} ]]; then
			echo "Retrying after ${interval} seconds (Attempt ${i} of $((max_tries - 1)))"
		else
			break
		fi
		sleep "${interval}"
	done
	set -e # Re-enable immediate exit on failure
	if [[ ${i} -eq ${max_tries} ]]; then
		die "Failed to deploy kata-deploy after ${max_tries} tries"
	fi

	# `helm install --wait` does not take effect on single replicas and maxUnavailable=1 DaemonSets
	# like kata-deploy on CI. So wait for pods being Running in the "traditional" way.
	local cmd
	cmd="kubectl -n kube-system get -l name=kata-deploy pod 2>/dev/null | grep '\<Running\>'"
	waitForProcess "${KATA_DEPLOY_WAIT_TIMEOUT}" 10 "${cmd}"

	# FIXME: This is needed as the kata-deploy pod will be set to "Ready"
	# when it starts running, which may cause issues like not having the
	# node properly labeled or the artefacts properly deployed when the
	# tests actually start running.
	sleep 60s

	echo "::group::kata-deploy logs"
	kubectl_retry -n kube-system logs --tail=100 -l name=kata-deploy
	echo "::endgroup::"

	echo "::group::Runtime classes"
	kubectl_retry get runtimeclass
	echo "::endgroup::"
}
