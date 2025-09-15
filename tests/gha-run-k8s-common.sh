#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

tests_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${tests_dir}/common.bash"
kubernetes_dir="${tests_dir}/integration/kubernetes"
helm_chart_dir="${repo_root_dir}/tools/packaging/kata-deploy/helm-chart/kata-deploy"

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
KATA_DEPLOY_WAIT_TIMEOUT="${KATA_DEPLOY_WAIT_TIMEOUT:-600}"
KATA_HOST_OS="${KATA_HOST_OS:-}"
KUBERNETES="${KUBERNETES:-}"
K8S_TEST_HOST_TYPE="${K8S_TEST_HOST_TYPE:-small}"
TEST_CLUSTER_NAMESPACE="${TEST_CLUSTER_NAMESPACE:-}"
CONTAINER_RUNTIME="${CONTAINER_RUNTIME:-containerd}"

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
		metadata="${GH_PR_NUMBER}-${short_sha}-${KATA_HYPERVISOR}-${KATA_HOST_OS}-amd64-${K8S_TEST_HOST_TYPE:0:1}-${GENPOLICY_PULL_METHOD:0:1}"
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
		"K8S_TEST_HOST_TYPE=${K8S_TEST_HOST_TYPE:0:1}" \
		"GENPOLICY_PULL_METHOD=${GENPOLICY_PULL_METHOD:0:1}")

	az group create \
		-l eastus \
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
		--tags "${tags[@]}"
}

function install_bats() {
	# Installing bats from the noble repo.
	sudo apt install -y software-properties-common
	sudo add-apt-repository 'deb http://archive.ubuntu.com/ubuntu/ noble universe'
	sudo apt install -y bats
	sudo add-apt-repository --remove 'deb http://archive.ubuntu.com/ubuntu/ noble universe'
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
	curl -sSLf -sSLf https://get.k0s.sh | sudo sh

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

	echo "${crio_version}"
}

function setup_crio() {
	# Get the CRI-O version to be installed depending on the version of the
	# "k8s distro" that we are using
	case "${KUBERNETES}" in
		k0s) crio_version=$(_get_k0s_kubernetes_version_for_crio) ;;
		*) >&2 echo "${KUBERNETES} flavour is not supported with CRI-O"; exit 2 ;;

	esac

	install_crio "${crio_version}"
}

function deploy_k8s() {
	echo "::group::Deploying ${KUBERNETES}"

	case "${KUBERNETES}" in
		k0s) deploy_k0s ;;
		k3s) deploy_k3s ;;
		rke2) deploy_rke2 ;;
		microk8s) deploy_microk8s ;;
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

	values_yaml=$(mktemp -t values_yaml.XXXXXX)

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
		[[ -n "${HELM_DEBUG}" ]] && yq -i ".env.debug = \"${HELM_DEBUG}\"" "${values_yaml}"
		[[ -n "${HELM_SHIMS}" ]] && yq -i ".env.shims = \"${HELM_SHIMS}\"" "${values_yaml}"
		[[ -n "${HELM_DEFAULT_SHIM}" ]] && yq -i ".env.defaultShim = \"${HELM_DEFAULT_SHIM}\"" "${values_yaml}"
		[[ -n "${HELM_CREATE_RUNTIME_CLASSES}" ]] && yq -i ".env.createRuntimeClasses = \"${HELM_CREATE_RUNTIME_CLASSES}\"" "${values_yaml}"
		[[ -n "${HELM_CREATE_DEFAULT_RUNTIME_CLASS}" ]] && yq -i ".env.createDefaultRuntimeClass = \"${HELM_CREATE_DEFAULT_RUNTIME_CLASS}\"" "${values_yaml}"
		[[ -n "${HELM_ALLOWED_HYPERVISOR_ANNOTATIONS}" ]] && yq -i ".env.allowedHypervisorAnnotations = \"${HELM_ALLOWED_HYPERVISOR_ANNOTATIONS}\"" "${values_yaml}"
		[[ -n "${HELM_SNAPSHOTTER_HANDLER_MAPPING}" ]] && yq -i ".env.snapshotterHandlerMapping = \"${HELM_SNAPSHOTTER_HANDLER_MAPPING}\"" "${values_yaml}"
		[[ -n "${HELM_AGENT_HTTPS_PROXY}" ]] && yq -i ".env.agentHttpsProxy = \"${HELM_AGENT_HTTPS_PROXY}\"" "${values_yaml}"
		[[ -n "${HELM_AGENT_NO_PROXY}" ]] && yq -i ".env.agentNoProxy = \"${HELM_AGENT_NO_PROXY}\"" "${values_yaml}"
		[[ -n "${HELM_PULL_TYPE_MAPPING}" ]] && yq -i ".env.pullTypeMapping = \"${HELM_PULL_TYPE_MAPPING}\"" "${values_yaml}"
		[[ -n "${HELM_HOST_OS}" ]] && yq -i ".env.hostOS=\"${HELM_HOST_OS}\"" "${values_yaml}"
	fi

	echo "::group::Final kata-deploy manifests used in the test"
	cat "${values_yaml}"
	echo ""
	helm template "${helm_chart_dir}" --values "${values_yaml}" --namespace kube-system
	[[ "$(yq .image.reference "${values_yaml}")" = "${HELM_IMAGE_REFERENCE}" ]] || die "Failed to set image reference"
	[[ "$(yq .image.tag "${values_yaml}")" = "${HELM_IMAGE_TAG}" ]] || die "Failed to set image tag"
	echo "::endgroup::"

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
