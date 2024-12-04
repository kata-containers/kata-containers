#!/usr/bin/env bash

# Copyright (c) 2024 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# Provides a library to deal with the CoCo KBS
#

kubernetes_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=1091
source "${kubernetes_dir}/../../gha-run-k8s-common.sh"
# shellcheck disable=1091
source "${kubernetes_dir}/../../../tests/common.bash"
source "${kubernetes_dir}/../../../tools/packaging/guest-image/lib_se.sh"
# For kata-runtime
export PATH="${PATH}:/opt/kata/bin"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
ITA_KEY="${ITA_KEY:-}"
HTTPS_PROXY="${HTTPS_PROXY:-}"
# Where the trustee (includes kbs) sources will be cloned
readonly COCO_TRUSTEE_DIR="/tmp/trustee"
# Where the kbs sources will be cloned
readonly COCO_KBS_DIR="${COCO_TRUSTEE_DIR}/kbs"
# The k8s namespace where the kbs service is deployed
readonly KBS_NS="coco-tenant"
# The private key file used for CLI authentication
readonly KBS_PRIVATE_KEY="${KBS_PRIVATE_KEY:-/opt/trustee/install/kbs.key}"
# The kbs service name
readonly KBS_SVC_NAME="kbs"
# The kbs ingress name
readonly KBS_INGRESS_NAME="kbs"

# Set "allow all" policy to resources.
#
kbs_set_allow_all_resources() {
	kbs_set_resources_policy \
		"${COCO_KBS_DIR}/sample_policies/allow_all.rego"
}

# Set "deny all" policy to resources.
#
kbs_set_deny_all_resources() {
	kbs_set_resources_policy \
		"${COCO_KBS_DIR}/sample_policies/deny_all.rego"
}

# Set resources policy.
#
# Parameters:
#	$1 - path to policy file
#
kbs_set_resources_policy() {
	local file="${1:-}"

	if [ ! -f "$file" ]; then
		>&2 echo "ERROR: policy file '$file' does not exist"
		return 1
	fi

	kbs-client --url "$(kbs_k8s_svc_http_addr)" config \
		--auth-private-key "$KBS_PRIVATE_KEY" set-resource-policy \
		--policy-file "$file"
}

# Set resource data in base64 encoded.
#
# Parameters:
#	$1 - repository name (optional)
#	$2 - resource type (mandatory)
#	$3 - tag (mandatory)
#	$4 - resource data in base64
#
kbs_set_resource_base64() {
	local repository="${1:-}"
	local type="${2:-}"
	local tag="${3:-}"
	local data="${4:-}"
	local file
	local rc=0

	if [ -z "$data" ]; then
		>&2 echo "ERROR: missing data parameter"
		return 1
	fi

	file=$(mktemp -t kbs-resource-XXXXX)
	echo "$data" | base64 -d > "$file"

	kbs_set_resource_from_file "$repository" "$type" "$tag" "$file" || \
		rc=$?

	rm -f "$file"
	return $rc
}

# Set resource data.
#
# Parameters:
#	$1 - repository name (optional)
#	$2 - resource type (mandatory)
#	$3 - tag (mandatory)
#	$4 - resource data
#
kbs_set_resource() {
	local repository="${1:-}"
	local type="${2:-}"
	local tag="${3:-}"
	local data="${4:-}"
	local file
	local rc=0

	if [ -z "$data" ]; then
		>&2 echo "ERROR: missing data parameter"
		return 1
	fi

	file=$(mktemp -t kbs-resource-XXXXX)
	echo "$data" > "$file"

	kbs_set_resource_from_file "$repository" "$type" "$tag" "$file" || \
		rc=$?

	rm -f "$file"
	return $rc
}

# Set resource, read data from file.
#
# Parameters:
#	$1 - repository name (optional)
#	$2 - resource type (mandatory)
#	$3 - tag (mandatory)
#	$4 - resource data
#
kbs_set_resource_from_file() {
	local repository="${1:-}"
	local type="${2:-}"
	local tag="${3:-}"
	local file="${4:-}"

	if [[ -z "$type" || -z "$tag" ]]; then
		>&2 echo "ERROR: missing type='$type' and/or tag='$tag' parameters"
		return 1
	elif [ ! -f "$file" ]; then
		>&2 echo "ERROR: resource file '$file' does not exist"
		return 1
	fi

	local path=""
	[ -n "$repository" ] && path+="${repository}/"
	path+="${type}/"
	path+="${tag}"

	kbs-client --url "$(kbs_k8s_svc_http_addr)" config \
		--auth-private-key "$KBS_PRIVATE_KEY" set-resource \
		--path "$path" --resource-file "$file"

	kbs_pod=$(kubectl -n $KBS_NS get pods -o NAME)
	kbs_repo_path="/opt/confidential-containers/kbs/repository"
	# Waiting for the resource to be created on the kbs pod
	if ! kubectl -n $KBS_NS exec -it "$kbs_pod" -- bash -c "for i in {1..30}; do [ -e '$kbs_repo_path/$path' ] && exit 0; sleep 0.5; done; exit -1"; then
		echo "ERROR: resource '$path' not created in 15s"
		kubectl -n $KBS_NS exec -it "$kbs_pod" -- bash -c "find $kbs_repo_path"
		return 1
	fi
}

# Build and install the kbs-client binary, unless it is already present.
#
kbs_install_cli() {
	command -v kbs-client >/dev/null && return

	source /etc/os-release || source /usr/lib/os-release
	case "${ID}" in
		ubuntu)
			local pkgs="build-essential pkg-config libssl-dev"

			sudo apt-get update -y
			# shellcheck disable=2086
			sudo apt-get install -y $pkgs
			;;
		centos)
			local pkgs="make"

			# shellcheck disable=2086
			sudo dnf install -y $pkgs
			;;
		*)
			>&2 echo "ERROR: running on unsupported distro"
			return 1
			;;
	esac

	# Mininum required version to build the client (read from versions.yaml)
	local rust_version
	ensure_yq
	rust_version=$(get_from_kata_deps ".externals.coco-trustee.toolchain")
	# Currently kata version from version.yaml is 1.72.0
	# which doesn't match the requirement, so let's pass
	# the required version.
	_ensure_rust "$rust_version"

	pushd "${COCO_KBS_DIR}"
	# Compile with sample features to bypass attestation.
	make CLI_FEATURES=sample_only cli
	sudo make install-cli
	popd
}

kbs_uninstall_cli() {
	if [ -d "${COCO_KBS_DIR}" ]; then
		pushd "${COCO_KBS_DIR}"
		sudo make uninstall
		popd
	else
		echo "${COCO_KBS_DIR} does not exist in the machine, skip uninstalling the kbs cli"
	fi
}

# Delete the kbs on Kubernetes
#
# Note: assume the kbs sources were cloned to $COCO_TRUSTEE_DIR
#
function kbs_k8s_delete() {
	pushd "$COCO_KBS_DIR"
	if [ "${KATA_HYPERVISOR}" = "qemu-tdx" ]; then
		kubectl delete -k config/kubernetes/ita
	elif [ "${KATA_HYPERVISOR}" = "qemu-se" ]; then
		kubectl delete -k config/kubernetes/overlays/ibm-se
	else
		kubectl delete -k config/kubernetes/overlays/
	fi

	# Verify that KBS namespace resources were properly deleted
	cmd="kubectl get all -n $KBS_NS 2>&1 | grep 'No resources found'"
	waitForProcess "120" "30" "$cmd"
	popd
}

# Deploy the kbs on Kubernetes
#
# Parameters:
#	$1 - apply the specificed ingress handler to expose the service externally
#
function kbs_k8s_deploy() {
	local image
	local image_tag
	local ingress=${1:-}
	local repo
	local svc_host
	local timeout
	local kbs_ip
	local kbs_port
	local version

	# yq is needed by get_from_kata_deps
	ensure_yq

	# Read from versions.yaml
	repo=$(get_from_kata_deps ".externals.coco-trustee.url")
	version=$(get_from_kata_deps ".externals.coco-trustee.version")
	image=$(get_from_kata_deps ".externals.coco-trustee.image")
	image_tag=$(get_from_kata_deps ".externals.coco-trustee.image_tag")

	# Image tag for TDX
	if [ "${KATA_HYPERVISOR}" = "qemu-tdx" ]; then
		image=$(get_from_kata_deps ".externals.coco-trustee.ita_image")
		image_tag=$(get_from_kata_deps ".externals.coco-trustee.ita_image_tag")
	fi

	# The ingress handler for AKS relies on the cluster's name which in turn
	# contain the HEAD commit of the kata-containers repository (supposedly the
	# current directory). It will be needed to save the cluster's name before
	# it switches to the kbs repository and get a wrong HEAD commit.
	if [ -z "${AKS_NAME:-}" ]; then
		AKS_NAME=$(_print_cluster_name)
		export AKS_NAME
	fi

	if [ -d "$COCO_TRUSTEE_DIR" ]; then
		rm -rf "$COCO_TRUSTEE_DIR"
	fi

	echo "::group::Clone the kbs sources"
	git clone --depth 1 "${repo}" "$COCO_TRUSTEE_DIR"
	pushd "$COCO_TRUSTEE_DIR"
	git fetch --depth=1 origin "${version}"
	git checkout FETCH_HEAD -b kbs_$$
	popd
	echo "::endgroup::"

	pushd "${COCO_KBS_DIR}/config/kubernetes/"

	# Tests should fill kbs resources later, however, the deployment
	# expects at least one secret served at install time.
	echo "somesecret" > overlays/key.bin

	# For qemu-se runtime, prepare the necessary resources
	if [ "${KATA_HYPERVISOR}" == "qemu-se" ]; then
		mv overlays/key.bin overlays/ibm-se/key.bin
		prepare_credentials_for_qemu_se
		# SE_SKIP_CERTS_VERIFICATION should be set to true
		# to skip the verification of the certificates
		sed -i "s/false/true/g" overlays/ibm-se/patch.yaml
	fi

	echo "::group::Update the kbs container image"
	install_kustomize
	pushd base
	kustomize edit set image "kbs-container-image=${image}:${image_tag}"
	popd
	echo "::endgroup::"
	[ -n "$ingress" ] && _handle_ingress "$ingress"

	echo "::group::Deploy the KBS"
	if [ "${KATA_HYPERVISOR}" = "qemu-tdx" ]; then
		echo "::group::Setting up ITA/ITTS for TDX"
		pushd "${COCO_KBS_DIR}/config/kubernetes/ita/"
			# Let's replace the "tBfd5kKX2x9ahbodKV1..." sample
			# `api_key`property by a valid ITA/ITTS API key, in the
			# ITA/ITTS specific configuration
			sed -i -e "s/tBfd5kKX2x9ahbodKV1.../${ITA_KEY}/g" kbs-config.toml
		popd
		
		if [ -n "${HTTPS_PROXY}" ]; then
			# Ideally this should be something kustomizable on trustee side.
			#
			# However, for now let's take the bullet and do it here, and revert this as
			# soon as https://github.com/confidential-containers/trustee/issues/567 is
			# solved.
			pushd "${COCO_KBS_DIR}/config/kubernetes/base/"
				ensure_yq
				
				yq e ".spec.template.spec.containers[0].env += [{\"name\": \"https_proxy\", \"value\": \"$HTTPS_PROXY\"}]" -i deployment.yaml
			popd
		fi

		export DEPLOYMENT_DIR=ita
	fi

	./deploy-kbs.sh

	# Check the private key used to install the KBS exist and save it in a
	# well-known location. That's the access key used by the kbs-client.
	local install_key="${PWD}/base/kbs.key"
	if [ ! -f "$install_key" ]; then
		echo "ERROR: KBS private key not found at ${install_key}"
		return 1
	fi
	sudo mkdir -p "$(dirname "$KBS_PRIVATE_KEY")"
	sudo cp -f "${install_key}" "$KBS_PRIVATE_KEY"

	popd

	if ! waitForProcess "120" "10" "kubectl -n \"$KBS_NS\" get pods | \
		grep -q '^kbs-.*Running.*'"; then
		echo "ERROR: KBS service pod isn't running"
		echo "::group::DEBUG - describe kbs deployments"
		kubectl -n "$KBS_NS" get deployments || true
		echo "::endgroup::"
		echo "::group::DEBUG - describe kbs pod"
		kubectl -n "$KBS_NS" describe pod -l app=kbs || true
		echo "::endgroup::"
		return 1
	fi
	echo "::endgroup::"

	# By default, the KBS service is reachable within the cluster only,
	# thus the following healthy checker should run from a pod. So start a
	# debug pod where it will try to get a response from the service. The
	# expected response is '404 Not Found' because it will request an endpoint
	# that does not exist.
	#
	echo "::group::Check the service healthy"
	kbs_ip=$(kubectl get -o jsonpath='{.spec.clusterIP}' svc "$KBS_SVC_NAME" -n "$KBS_NS" 2>/dev/null)
	kbs_port=$(kubectl get -o jsonpath='{.spec.ports[0].port}' svc "$KBS_SVC_NAME" -n "$KBS_NS" 2>/dev/null)

	local pod=kbs-checker-$$
	kubectl run "$pod" --image=quay.io/prometheus/busybox --restart=Never -- \
		sh -c "wget -O- --timeout=5 \"${kbs_ip}:${kbs_port}\" || true"
	if ! waitForProcess "60" "10" "kubectl logs \"$pod\" 2>/dev/null | grep -q \"404 Not Found\""; then
		echo "ERROR: KBS service is not responding to requests"
		echo "::group::DEBUG - kbs logs"
		kubectl -n "$KBS_NS" logs -l app=kbs || true
		echo "::endgroup::"
		kubectl delete pod "$pod"
		return 1
	fi
	kubectl delete pod "$pod"
	echo "KBS service respond to requests"
	echo "::endgroup::"

	if [ -n "$ingress" ]; then
		echo "::group::Check the kbs service is exposed"
		svc_host=$(kbs_k8s_svc_http_addr)
		if [ -z "$svc_host" ]; then
			echo "ERROR: service host not found"
			return 1
		fi

		# AZ DNS can take several minutes to update its records so that
		# the host name will take a while to start resolving.
		timeout=350
		echo "Trying to connect at $svc_host. Timeout=$timeout"
		if ! waitForProcess "$timeout" "30" "curl -s -I \"$svc_host\" | grep -q \"404 Not Found\""; then
			echo "ERROR: service seems to not respond on $svc_host host"
			curl -I "$svc_host"
			return 1
		fi
		echo "KBS service respond to requests at $svc_host"
		echo "::endgroup::"
	fi
}

# Return the kbs service host name in case ingress is configured
# otherwise the cluster IP.
#
kbs_k8s_svc_host() {
	if kubectl get ingress -n "$KBS_NS" 2>/dev/null | grep -q kbs; then
		kubectl get ingress "$KBS_INGRESS_NAME" -n "$KBS_NS" \
			-o jsonpath='{.spec.rules[0].host}' 2>/dev/null
	elif kubectl get svc "$KBS_SVC_NAME" -n "$KBS_NS" &>/dev/null; then
			local host
			host=$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' -n "$KBS_NS")
			echo "$host"
	else
		kubectl get svc "$KBS_SVC_NAME" -n "$KBS_NS" \
			-o jsonpath='{.spec.clusterIP}' 2>/dev/null
	fi
}

# Return the kbs service port number. In case ingress is configured
# it will return "80", otherwise the pod's service port.
#
kbs_k8s_svc_port() {
	if kubectl get ingress -n "$KBS_NS" 2>/dev/null | grep -q kbs; then
		# Assume served on default HTTP port 80
		echo "80"
	elif kubectl get svc "$KBS_SVC_NAME" -n "$KBS_NS" &>/dev/null; then
		kubectl get svc "$KBS_SVC_NAME" -n "$KBS_NS" -o jsonpath='{.spec.ports[0].nodePort}'
	else
		kubectl get svc "$KBS_SVC_NAME" -n "$KBS_NS" \
			-o jsonpath='{.spec.ports[0].port}' 2>/dev/null
	fi
}

# Return the kbs service HTTP address (http://host:port).
#
kbs_k8s_svc_http_addr() {
	local host
	local port

	host=$(kbs_k8s_svc_host)
	port=$(kbs_k8s_svc_port)

	echo "http://${host}:${port}"
}

# Ensure rust is installed in the host.
#
# It won't install rust if it's already present, however, if the current
# version isn't greater or equal than the mininum required then it will
# bail out with an error.
#
_ensure_rust() {
	rust_version=${1:-}

	if ! command -v rustc >/dev/null; then
		"${kubernetes_dir}/../../install_rust.sh" "${rust_version}"

		# shellcheck disable=1091
		source "$HOME/.cargo/env"
	else
		[ -z "$rust_version" ] && return

		# We don't want to mess with installation on bare-metal so
		# if rust is installed then just check it's >= the required
		# version.
		#
		local current_rust_version
		current_rust_version="$(rustc --version | cut -d' ' -f2)"
		if ! version_greater_than_equal "${current_rust_version}" \
			"${rust_version}"; then
			>&2 echo "ERROR: installed rust $current_rust_version < $rust_version (required)"
			return 1
		fi
	fi
}

# Choose the appropriated ingress handler.
#
# To add a new handler, create a function named as _handle_ingress_NAME where
# NAME is the handler name. This is enough for this method to pick up the right
# implementation.
#
_handle_ingress() {
	local ingress="$1"

	type -a "_handle_ingress_$ingress" &>/dev/null || {
		echo "ERROR: ingress '$ingress' handler not implemented";
		return 1;
	}

	"_handle_ingress_$ingress"
}

# Implement the ingress handler for AKS.
#
_handle_ingress_aks() {
	local dns_zone

	dns_zone=$(get_cluster_specific_dns_zone "")

	# In case the DNS zone name is empty, the cluster might not have the HTTP
	# application routing add-on. Let's try to enable it.
	if [ -z "$dns_zone" ]; then
		echo "::group::Enable HTTP application routing add-on"
		enable_cluster_http_application_routing ""
		echo "::endgroup::"
		dns_zone=$(get_cluster_specific_dns_zone "")
	fi

	if [ -z "$dns_zone" ]; then
		echo "ERROR: the DNS zone name is nil, it cannot configure Ingress"
		return 1
	fi

	pushd "${COCO_KBS_DIR}/config/kubernetes/overlays/"

	echo "::group::$(pwd)/ingress.yaml"
	KBS_INGRESS_CLASS="addon-http-application-routing" \
		KBS_INGRESS_HOST="kbs.${dns_zone}" \
		envsubst < ingress.yaml | tee ingress.yaml.tmp
	echo "::endgroup::"
	mv ingress.yaml.tmp ingress.yaml

	kustomize edit add resource ingress.yaml
	popd
}

# Implements the ingress handler for servernode
#
_handle_ingress_nodeport() {
	# By exporting this variable the kbs deploy script will install the nodeport service
	export DEPLOYMENT_DIR=nodeport
}


# Prepare necessary resources for qemu-se runtime
# Documentation: https://github.com/confidential-containers/trustee/tree/main/attestation-service/verifier/src/se
prepare_credentials_for_qemu_se() {
	echo "::group::Prepare credentials for qemu-se runtime"
	if [ -z "${IBM_SE_CREDS_DIR:-}" ]; then
		>&2 echo "ERROR: IBM_SE_CREDS_DIR is empty"
		return 1
	fi
	config_file_path="/opt/kata/share/defaults/kata-containers/configuration-qemu-se.toml"
	kata_base_dir=$(dirname $(kata-runtime --config ${config_file_path} env --json | jq -r '.Kernel.Path'))
	if [ ! -d ${HKD_PATH} ]; then
		>&2 echo "ERROR: HKD_PATH is not set"
		return 1
	fi
	pushd "${IBM_SE_CREDS_DIR}"
	mkdir {certs,crls,hdr,hkds,rsa}
	openssl genrsa -aes256 -passout pass:test1234 -out encrypt_key-psw.pem 4096
	openssl rsa -in encrypt_key-psw.pem -passin pass:test1234 -pubout -out rsa/encrypt_key.pub
	openssl rsa -in encrypt_key-psw.pem -passin pass:test1234 -out rsa/encrypt_key.pem
	cp ${kata_base_dir}/kata-containers-se.img hdr/hdr.bin
	cp ${HKD_PATH}/HKD-*.crt hkds/
	cp ${HKD_PATH}/ibm-z-host-key-gen2.crl crls/
	cp ${HKD_PATH}/DigiCertCA.crt ${HKD_PATH}/ibm-z-host-key-signing-gen2.crt certs/
	popd
	ls -R ${IBM_SE_CREDS_DIR}
	echo "::endgroup::"
}
