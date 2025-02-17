#!/usr/bin/env bash

# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

tests_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${tests_dir}/common.bash"

K8S_TEST_HOST_TYPE="${K8S_TEST_HOST_TYPE:-small}"
GH_PR_NUMBER="${GH_PR_NUMBER:-}"
GENPOLICY_PULL_METHOD="${GENPOLICY_PULL_METHOD:-oci-distribution}"

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

# Print the cluster name set by $AKS_NAME or generated out of runtime
# metadata (e.g. pull request number, commit SHA, etc).
#
function _print_cluster_name() {
	local test_type="${1:-k8s}"
	local short_sha
	local metadata

	if [ -n "${AKS_NAME:-}" ]; then
		echo "$AKS_NAME"
	else
		short_sha="$(git rev-parse --short=12 HEAD)"
		metadata="${GH_PR_NUMBER}-${short_sha}-${KATA_HYPERVISOR}-${KATA_HOST_OS}-amd64-${K8S_TEST_HOST_TYPE:0:1}-${GENPOLICY_PULL_METHOD:0:1}"
		# Compute the SHA1 digest of the metadata part to keep the name less
		# than the limit of 63 chars of AKS
		echo "${test_type}-$(sha1sum <<< "$metadata" | cut -d' ' -f1)"
	fi
}

function _print_rg_name() {
	test_type="${1:-k8s}"

	echo "${AZ_RG:-"kataCI-$(_print_cluster_name ${test_type})"}"
}

# Enable the HTTP application routing add-on to AKS.
# Use with ingress to expose a service API externally.
#
function enable_cluster_http_application_routing() {
	local test_type="${1:-k8s}"
	local cluster_name
	local rg

	rg="$(_print_rg_name "${test_type}")"
	cluster_name="$(_print_cluster_name "${test_type}")"

	az aks enable-addons -g "$rg" -n "$cluster_name" \
		--addons http_application_routing
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

	# Switch to the Kata Containers subscription
	az account set --subscription "${AZ_SUBSCRIPTION_ID}"
}

function create_cluster() {
	test_type="${1:-k8s}"
	local short_sha
	local tags

	# First ensure it didn't fail to get cleaned up from a previous run.
	delete_cluster "${test_type}" || true

	local rg="$(_print_rg_name ${test_type})"

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

	az aks create \
		-g "${rg}" \
		--node-resource-group "node-${rg}" \
		-n "$(_print_cluster_name ${test_type})" \
		-s "$(_print_instance_type)" \
		--node-count 1 \
		--generate-ssh-keys \
		--tags "${tags[@]}" \
		$([ "${KATA_HOST_OS}" = "cbl-mariner" ] && echo "--os-sku AzureLinux --workload-runtime KataMshvVmIsolation")
}

function install_bats() {
	# Installing bats from the noble repo.
	sudo apt install -y software-properties-common
	sudo add-apt-repository 'deb http://archive.ubuntu.com/ubuntu/ noble universe'
	sudo apt install -y bats
	sudo add-apt-repository --remove 'deb http://archive.ubuntu.com/ubuntu/ noble universe'
}

function install_kubectl() {
	sudo az aks install-cli
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
	curl -Lf -o "$tarball" "https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize/${version}/${tarball}"

	local rc=0
	echo "${checksum} $tarball" | sha256sum -c || rc=$?
	[ $rc -eq 0 ] && sudo tar -xvzf "${tarball}" -C /usr/local/bin || rc=$?
	rm -f "$tarball"
	[ $rc -eq 0 ]
}

function get_cluster_credentials() {
	test_type="${1:-k8s}"

	az aks get-credentials \
		--overwrite-existing \
		-g "$(_print_rg_name ${test_type})" \
		-n "$(_print_cluster_name ${test_type})"
}


# Get the AKS DNS zone name of HTTP application routing.
#
# Note: if the HTTP application routing add-on isn't installed in the cluster
# then it will return an empty string.
#
function get_cluster_specific_dns_zone() {
	local test_type="${1:-k8s}"
	local cluster_name
	local rg
	local q="addonProfiles.httpApplicationRouting.config.HTTPApplicationRoutingZoneName"

	rg="$(_print_rg_name "${test_type}")"
	cluster_name="$(_print_cluster_name "${test_type}")"

	az aks show -g "$rg" -n "$cluster_name" --query "$q" | tr -d \"
}

function delete_cluster() {
	test_type="${1:-k8s}"
	local rg
	rg="$(_print_rg_name ${test_type})"

	if [ "$(az group exists -g "${rg}")" == "true" ]; then
		az group delete -g "${rg}" --yes
	fi
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
	sudo curl -fL --progress-bar -o /usr/bin/kubectl https://dl.k8s.io/release/${kubectl_version}/bin/linux/${ARCH}/kubectl
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
	ARCH=$(arch_to_golang)

	kubectl_version=$(/usr/local/bin/k3s kubectl version --client=true 2>/dev/null | grep "Client Version" | sed -e 's/Client Version: //' -e 's/+k3s[0-9]\+//')
	sudo curl -fL --progress-bar -o /usr/bin/kubectl https://dl.k8s.io/release/${kubectl_version}/bin/linux/${ARCH}/kubectl
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
		pids=$(pgrep -f ${script_name})
		if [ -n "$pids" ]; then
			echo "$pids" | xargs sudo kill -SIGTERM >/dev/null 2>&1 || true
		fi
	done
}
