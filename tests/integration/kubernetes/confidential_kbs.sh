#!/usr/bin/env bash

# Copyright (c) 2024 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# Provides a library to deal with the CoCo KBS
#
set -e

kubernetes_dir="${kubernetes_dir:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
# shellcheck disable=1091
source "${kubernetes_dir}/../../gha-run-k8s-common.sh"
# shellcheck disable=1091
source "${kubernetes_dir}/../../../tests/common.bash"
# shellcheck disable=1091
source "${kubernetes_dir}/../../../tools/packaging/guest-image/lib_se.sh"
# For kata-runtime
export PATH="${PATH}:/opt/kata/bin"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-runtime-rs}"
# Only the Attestation Service reaches out to the public Intel DCAP collateral
# endpoint (api.trustedservices.intel.com) when verifying TDX quotes, so on the
# Intel TDX CI runner it needs the corporate HTTPS proxy. HTTP_PROXY is
# deliberately ignored: the KBS <-> AS <-> RVPS gRPC/HTTP traffic is in-cluster
# and routing it through the proxy breaks those connections.
HTTPS_PROXY="${HTTPS_PROXY:-${https_proxy:-}}"
# Where the trustee (includes kbs) sources will be cloned
readonly COCO_TRUSTEE_DIR="/tmp/trustee"
# Where the kbs sources will be cloned
readonly COCO_KBS_DIR="${COCO_TRUSTEE_DIR}/kbs"
# The Helm chart directory inside the trustee repo
readonly COCO_HELM_CHART_DIR="${COCO_TRUSTEE_DIR}/deployment/helm-chart"
# The Helm release name
readonly HELM_RELEASE_NAME="trustee"
# The admin bearer-token file used for kbs-client authentication. The chart's
# bootstrap job mints this JWT (signed with the admin key) and stores it in the
# bootstrap secret; KBS runs with authorization_mode = "AuthenticatedAuthorization",
# so admin requests must carry it.
readonly KBS_ADMIN_TOKEN_FILE="${KBS_ADMIN_TOKEN_FILE:-/opt/trustee/install/kbs-admin-token}"
# The bootstrap secret name holding admin keys
readonly KBS_BOOTSTRAP_SECRET="trustee-bootstrap-user-keys"

# The k8s namespace where the kbs service is deployed
readonly KBS_NS="coco-trustee"
# The kbs service name (Helm chart names it <release>-kbs)
readonly KBS_SVC_NAME="trustee-kbs"
# The kbs service exposed via NodePort (Helm chart names it <svc>-nodeport)
readonly KBS_NODEPORT_SVC_NAME="trustee-kbs-nodeport"
# The kbs ingress name
readonly KBS_INGRESS_NAME="trustee-kbs"
# Workdir for installing snphost
readonly SNPHOST_DIR="/tmp/snphost-workdir"

# Set "allow all" policy to resources.
#
kbs_set_allow_all_resources() {
	kbs_set_resources_policy \
		"${COCO_KBS_DIR}/sample_policies/allow_all.rego"
}

kbs_set_default_policy() {
	kbs_set_resources_policy \
		"${COCO_KBS_DIR}/sample_policies/default.rego"
}

# Set "deny all" policy to resources.
#
kbs_set_deny_all_resources() {
	kbs_set_resources_policy \
		"${COCO_KBS_DIR}/sample_policies/deny_all.rego"
}

# Set KBS resource policy requiring GPU0's EAR status to be non-contraindicated.
#
kbs_set_gpu0_resource_policy() {
	local policy_file
	policy_file=$(mktemp -t kbs-gpu-policy-XXXXX.rego)

	cat > "${policy_file}" <<-'EOF'
		package policy
		import rego.v1
		default allow = false
		allow if {
		    input["submods"]["gpu0"]["ear.status"] == "affirming"
		}
	EOF

	kbs_set_resources_policy "${policy_file}"
	local rc=$?
	rm -f "${policy_file}"
	return "${rc}"
}

# Set KBS resource policy requiring CPU0's EAR status to be affirming.
#
kbs_set_cpu0_resource_policy() {
	local policy_file
	policy_file=$(mktemp -t kbs-cpu-policy-XXXXX.rego)

	cat > "${policy_file}" <<-'EOF'
		package policy
		import rego.v1
		default allow = false
		allow if {
		    input["submods"]["cpu0"]["ear.status"] == "affirming"
		}
	EOF

	kbs_set_resources_policy "${policy_file}"
	local rc=$?
	rm -f "${policy_file}"
	return "${rc}"
}

# Set resources policy.
#
# Parameters:
#	$1 - path to policy file
#
kbs_set_resources_policy() {
	local file="${1:-}"

	if [[ ! -f "${file}" ]]; then
		>&2 echo "ERROR: policy file '${file}' does not exist"
		return 1
	fi

	kbs-client --url "$(kbs_k8s_svc_http_addr)" config \
		--admin-token-file "${KBS_ADMIN_TOKEN_FILE}" \
		set-resource-policy \
		--policy-file "${file}"
}

# Execute an admin command via the KBS client.
#
# Admin requests are authenticated with the bearer token minted by the chart's
# bootstrap job (KBS runs with authorization_mode = "AuthenticatedAuthorization").
#
# Parameters:
#	$1 - config command to run
#
kbs_config_command() {
	kbs-client --url "$(kbs_k8s_svc_http_addr)" config \
		--admin-token-file "${KBS_ADMIN_TOKEN_FILE}" \
		"$@"
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

	if [[ -z "${data}" ]]; then
		>&2 echo "ERROR: missing data parameter"
		return 1
	fi

	file=$(mktemp -t kbs-resource-XXXXX)
	echo "${data}" | base64 -d > "${file}"

	kbs_set_resource_from_file "${repository}" "${type}" "${tag}" "${file}" || \
		rc=$?

	rm -f "${file}"
	return "${rc}"
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

	if [[ -z "${data}" ]]; then
		>&2 echo "ERROR: missing data parameter"
		return 1
	fi

	file=$(mktemp -t kbs-resource-XXXXX)
	echo "${data}" > "${file}"

	kbs_set_resource_from_file "${repository}" "${type}" "${tag}" "${file}" || \
		rc=$?

	rm -f "${file}"
	return "${rc}"
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

	if [[ -z "${type}" || -z "${tag}" ]]; then
		>&2 echo "ERROR: missing type='${type}' and/or tag='${tag}' parameters"
		return 1
	elif [[ ! -f "${file}" ]]; then
		>&2 echo "ERROR: resource file '${file}' does not exist"
		return 1
	fi

	local path=""
	[[ -n "${repository}" ]] && path+="${repository}/"
	path+="${type}/"
	path+="${tag}"

	kbs-client --url "$(kbs_k8s_svc_http_addr)" config \
		--admin-token-file "${KBS_ADMIN_TOKEN_FILE}" \
		set-resource \
		--path "${path}" --resource-file "${file}"
}

# Build and install the kbs-client binary, unless a compatible one is already
# present.
#
# Reused (self-hosted / baremetal) CI runners can carry a kbs-client left over
# from an older Trustee whose admin API predates the bearer-token flag this
# script relies on. Reusing it makes every 'config' command fail with
# "unexpected argument '--admin-token-file'". Only skip the (re)build when the
# installed client actually understands --admin-token-file; otherwise rebuild it
# from the pinned Trustee sources cloned by kbs_k8s_deploy().
#
kbs_install_cli() {
	if command -v kbs-client >/dev/null; then
		if kbs-client config --help 2>&1 | grep -q -- '--admin-token-file'; then
			return
		fi
		echo "Existing kbs-client lacks --admin-token-file; rebuilding from ${COCO_KBS_DIR}"
	fi

	source /etc/os-release || source /usr/lib/os-release
	case "${ID}" in
		debian|ubuntu)
			local pkgs="build-essential pkg-config libssl-dev"

			sudo apt-get update -y
			# shellcheck disable=2086
			sudo apt-get install -y ${pkgs}
			;;
		centos)
			local pkgs="make"

			# shellcheck disable=2086,2248
			sudo dnf install -y ${pkgs}
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
	_ensure_rust "${rust_version}"

	pushd "${COCO_KBS_DIR}"
	# Compile with sample features to bypass attestation.
	make CLI_FEATURES=sample_only cli
	sudo make install-cli
	popd
}

kbs_uninstall_cli() {
	if [[ -d "${COCO_KBS_DIR}" ]]; then
		pushd "${COCO_KBS_DIR}"
		sudo make uninstall
		popd
	else
		echo "${COCO_KBS_DIR} does not exist in the machine, skip uninstalling the kbs cli"
	fi
}

# Ensure ~/.cicd/venv exists and activate it in the current shell.
ensure_cicd_python_venv() {
	local venv_path="${HOME}/.cicd/venv"
	if [[ ! -f "${venv_path}/bin/activate" ]]; then
		# NIM tests need Python 3.10 via pyenv; attestation uses system python3. Both are fine.
		if command -v pyenv &>/dev/null; then
			export PYENV_ROOT="${HOME}/.pyenv"
			[[ -d "${PYENV_ROOT}/bin" ]] && export PATH="${PYENV_ROOT}/bin:${PATH}"
			eval "$(pyenv init - bash)"
		fi
		mkdir -p "${HOME}/.cicd"
		python3 -m venv "${venv_path}"
	fi
	# shellcheck disable=SC1091
	source "${venv_path}/bin/activate"
}

# Ensure the sev-snp-measure utility is installed.
#
ensure_sev_snp_measure() {
	command -v sev-snp-measure >/dev/null && return

	ensure_cicd_python_venv
	pip install sev-snp-measure
}

# Ensure that snphost utility is installed
#
ensure_snphost() {
	command -v snphost >/dev/null && return

	git clone https://github.com/virtee/snphost.git "${SNPHOST_DIR}"
	pushd "${SNPHOST_DIR}"

	_ensure_rust "1.85.0"
	cargo build --release
	sudo install -m 755 target/release/snphost /usr/local/bin/

	popd
	rm -rf "${SNPHOST_DIR}"
}

# Delete the kbs on Kubernetes
#
function kbs_k8s_delete() {
	helm uninstall "${HELM_RELEASE_NAME}" -n "${KBS_NS}" || true

	# Verify that KBS namespace resources were properly deleted
	local cmd="kubectl get all -n ${KBS_NS} 2>&1 | grep 'No resources found'"
	waitForProcess "120" "30" "${cmd}"

	kubectl delete ns "${KBS_NS}" --ignore-not-found --wait=false
}

# Drop optional Trustee subchart dependencies that kata CI never enables.
#
# The upstream chart pins postgresql to Bitnami's Docker Hub OCI registry. Helm
# still requires `helm dependency build` even when postgresql.enabled=false.
# Our tests always use LocalFs/Memory storage, so remove the unused dependency
# and skip any remote chart fetch entirely.
#
# Parameters:
#	$1 - Trustee Helm chart directory
#
strip_trustee_unused_chart_dependencies() {
	local chart_dir=$1
	local chart_yaml="${chart_dir}/Chart.yaml"
	local chart_lock="${chart_dir}/Chart.lock"

	[[ -f "${chart_yaml}" ]] || die "Trustee Chart.yaml not found at ${chart_yaml}"

	ensure_yq
	yq -i 'del(.dependencies)' "${chart_yaml}"
	rm -f "${chart_lock}"
}

# Deploy the kbs on Kubernetes via the Trustee Helm chart.
#
# Parameters:
#	$1 - ingress type to expose the service externally (nodeport|aks|"")
#
# Environment (optional):
#	NVIDIA_VERIFIER_MODE - remote (default) | local: overrides the NVIDIA verifier
#	                       type when KATA_HYPERVISOR matches *nvidia-gpu*.
#
function kbs_k8s_deploy() {
	local ingress=${1:-}
	local repo
	local version
	local image_kbs
	local image_as
	local image_rvps
	local svc_host
	local timeout

	ensure_yq
	ensure_helm

	# Read from versions.yaml
	repo=$(get_from_kata_deps ".externals.coco-trustee.url")
	version=$(get_from_kata_deps ".externals.coco-trustee.version")
	image_kbs=$(get_from_kata_deps ".externals.coco-trustee.images.kbs")
	image_as=$(get_from_kata_deps ".externals.coco-trustee.images.as")
	image_rvps=$(get_from_kata_deps ".externals.coco-trustee.images.rvps")

	# The ingress handler for AKS relies on the cluster's name which in turn
	# contain the HEAD commit of the kata-containers repository (supposedly the
	# current directory). It will be needed to save the cluster's name before
	# it switches to the kbs repository and get a wrong HEAD commit.
	if [[ -z "${AKS_NAME:-}" ]]; then
		AKS_NAME=$(_print_cluster_name)
		export AKS_NAME
	fi

	if [[ -d "${COCO_TRUSTEE_DIR}" ]]; then
		rm -rf "${COCO_TRUSTEE_DIR}"
	fi

	echo "::group::Clone the trustee sources"
	git clone --depth 1 "${repo}" "${COCO_TRUSTEE_DIR}"
	pushd "${COCO_TRUSTEE_DIR}"
	git fetch --depth=1 origin "${version}"
	git checkout FETCH_HEAD -b trustee_$$
	popd
	echo "::endgroup::"

	# Image tags match the trustee source version.
	local kbs_repo="${image_kbs}"
	local kbs_tag="${version}"
	local as_repo="${image_as}"
	local as_tag="${version}"
	local rvps_repo="${image_rvps}"
	local rvps_tag="${version}"

	# Build Helm --set arguments for verifier configuration.
	# These supplement the values file and avoid embedding YAML inside the heredoc.
	#
	# The AS auto-selects a verifier per evidence type; the chart configures the
	# NVIDIA and Intel DCAP verifiers by default (DCAP already points at the
	# public Intel collateral service), so SNP and TDX need no extra knobs. Only
	# NVIDIA requires overriding the verifier type (Local vs Remote).
	local helm_set_args=()

	if [[ "${KATA_HYPERVISOR}" == *nvidia-gpu* ]]; then
		local nvidia_verifier_type
		nvidia_verifier_type="$(printf '%s' "${NVIDIA_VERIFIER_MODE:-remote}" | sed 's/./\u&/')"
		helm_set_args+=(--set "as.verifier.nvidia.type=${nvidia_verifier_type}")
	fi

	# Build Helm values override.
	#
	# The chart's default CPU/memory *requests* (kbs 250m, as 500m, rvps 100m)
	# reserve ~850m of CPU across the three Trustee Pods. On the small
	# single-node non-TEE CI runners that leaves too little allocatable CPU for
	# the test workloads (some request 2 full CPUs with Guaranteed QoS), so they
	# stay Pending with "Insufficient cpu". The builtin-AS kustomize deployment
	# ran a single Pod and did not reserve this much. Shrink the requests (which
	# is what the scheduler accounts for) to a minimum while leaving generous
	# limits so the AS can still burst during TDX quote verification.
	local values_file
	values_file=$(mktemp -t trustee-helm-values-XXXXX.yaml)
	cat > "${values_file}" <<-EOF
	# Enable debug logging on all Trustee components (kbs, as, rvps) to help
	# debug attestation / resource-retrieval failures in CI.
	log_level: debug
	kbs:
	  image:
	    repository: "${kbs_repo}"
	    tag: "${kbs_tag}"
	  service:
	    exposeLoadBalancer: false
	  resources:
	    requests:
	      cpu: 50m
	      memory: 128Mi
	    limits:
	      cpu: "1"
	      memory: 1Gi
	as:
	  image:
	    repository: "${as_repo}"
	    tag: "${as_tag}"
	  resources:
	    requests:
	      cpu: 50m
	      memory: 256Mi
	    limits:
	      cpu: "2"
	      memory: 2Gi
	rvps:
	  image:
	    repository: "${rvps_repo}"
	    tag: "${rvps_tag}"
	  resources:
	    requests:
	      cpu: 50m
	      memory: 64Mi
	    limits:
	      cpu: "1"
	      memory: 512Mi
	EOF

	# Handle ingress / nodeport
	if [[ "${ingress}" = "nodeport" ]]; then
		cat >> "${values_file}" <<-EOF
		nodePort:
		  enabled: true
		EOF
	elif [[ "${ingress}" = "aks" ]]; then
		echo "::group::Enable approuting (application routing) add-on"
		enable_cluster_approuting ""
		echo "::endgroup::"

		cat >> "${values_file}" <<-EOF
		ingress:
		  enabled: true
		  className: "webapprouting.kubernetes.azure.com"
		  host: ""
		EOF
	fi

	# Proxy handling (Intel TDX CI): only the Attestation Service reaches out to
	# the public Intel DCAP collateral endpoint (api.trustedservices.intel.com)
	# when verifying TDX quotes, so it is the only component that needs the
	# corporate proxy. We set https_proxy only: the KBS <-> AS <-> RVPS traffic is
	# plain HTTP/gRPC and in-cluster, and routing it through the proxy (via
	# HTTP_PROXY) breaks those connections. This mirrors the simple legacy setup,
	# which set https_proxy on the (then builtin) AS and no no_proxy at all.
	# Helm --set treats commas as key separators; escape them with \, for literal
	# commas.
	local as_env_idx=0
	if [[ -n "${HTTPS_PROXY}" ]]; then
		local helm_https_proxy="${HTTPS_PROXY//,/\\,}"
		helm_set_args+=(--set "as.extraEnvVars[${as_env_idx}].name=HTTPS_PROXY")
		helm_set_args+=(--set "as.extraEnvVars[${as_env_idx}].value=${helm_https_proxy}")
		as_env_idx=$((as_env_idx + 1))
		helm_set_args+=(--set "as.extraEnvVars[${as_env_idx}].name=https_proxy")
		helm_set_args+=(--set "as.extraEnvVars[${as_env_idx}].value=${helm_https_proxy}")
		as_env_idx=$((as_env_idx + 1))
	fi

	# Handle IBM SE (s390x): the SE verifier runs inside the gRPC Attestation
	# Service, so the SE materials are mounted on the AS Pod (not KBS) via the
	# chart's as.verifier.se.* knobs, which create a node-local PV/PVC pointing at
	# ${IBM_SE_CREDS_DIR} on the target node.
	if [[ "${KATA_HYPERVISOR}" == qemu-se* ]]; then
		local node_name
		node_name=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')
		prepare_credentials_for_qemu_se
		helm_set_args+=(--set "as.verifier.se.credsDir=${IBM_SE_CREDS_DIR:-}")
		helm_set_args+=(--set "as.verifier.se.nodeName=${node_name}")
		# fsGroup must match the owning GID of the files under credsDir so the
		# non-root AS container (CAP_DAC_OVERRIDE dropped) can read them.
		helm_set_args+=(--set "as.podSecurityContext.fsGroup=1000")
		# Development materials: skip the HKD certificate chain verification.
		helm_set_args+=(--set "as.extraEnvVars[${as_env_idx}].name=SE_SKIP_CERTS_VERIFICATION")
		helm_set_args+=(--set-string "as.extraEnvVars[${as_env_idx}].value=true")
		as_env_idx=$((as_env_idx + 1))
	fi

	# Baremetal / self-hosted CI clusters keep the same Kubernetes API; a prior
	# run may leave the Helm release secret behind (e.g. pending-install after a
	# timeout), which would make a subsequent upgrade fail. Clean it up so the
	# 'helm upgrade --install' below always starts from a known state.
	if helm status "${HELM_RELEASE_NAME}" -n "${KBS_NS}" &>/dev/null; then
		echo "Removing existing Helm release ${HELM_RELEASE_NAME} in namespace ${KBS_NS}"
		helm uninstall "${HELM_RELEASE_NAME}" -n "${KBS_NS}" --wait --timeout 5m
	fi

	# The chart declares a (condition-gated, disabled-by-default) postgresql
	# dependency pinned to Docker Hub OCI. CI never enables bundled Postgres, so
	# drop the dependency and avoid `helm dependency build` entirely.
	echo "::group::Prepare Trustee Helm chart"
	strip_trustee_unused_chart_dependencies "${COCO_HELM_CHART_DIR}"
	echo "::endgroup::"

	echo "::group::Deploy Trustee via Helm"
	echo "Helm values override:"
	cat "${values_file}"

	if ! helm upgrade --install "${HELM_RELEASE_NAME}" "${COCO_HELM_CHART_DIR}" \
		--namespace "${KBS_NS}" --create-namespace \
		-f "${values_file}" \
		"${helm_set_args[@]}" \
		--wait --timeout 5m --debug 2>&1; then
		echo "ERROR: helm upgrade --install failed"
		echo "::group::DEBUG - helm status"
		helm status "${HELM_RELEASE_NAME}" -n "${KBS_NS}" 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - all resources"
		kubectl -n "${KBS_NS}" get all 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - pods"
		kubectl -n "${KBS_NS}" get pods -o wide 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - describe pods"
		kubectl -n "${KBS_NS}" describe pods 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - services"
		kubectl -n "${KBS_NS}" get svc -o wide 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - endpoints"
		kubectl -n "${KBS_NS}" get endpoints 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - secrets"
		kubectl -n "${KBS_NS}" get secrets 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - deployments"
		kubectl -n "${KBS_NS}" get deployments -o wide 2>&1 || true
		kubectl -n "${KBS_NS}" describe deployments 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - jobs"
		kubectl -n "${KBS_NS}" get jobs 2>&1 || true
		kubectl -n "${KBS_NS}" describe jobs 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - events"
		kubectl -n "${KBS_NS}" get events --sort-by='.lastTimestamp' 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - kbs logs"
		kubectl -n "${KBS_NS}" logs -l app=kbs --all-containers 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - as logs"
		kubectl -n "${KBS_NS}" logs -l app=attestation-service --all-containers 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - rvps logs"
		kubectl -n "${KBS_NS}" logs -l app=reference-value-provider-service 2>&1 || true
		echo "::endgroup::"
		echo "::group::DEBUG - node status"
		kubectl get nodes -o wide 2>&1 || true
		kubectl describe nodes 2>&1 || true
		echo "::endgroup::"
		rm -f "${values_file}"
		return 1
	fi

	rm -f "${values_file}"

	# Extract the admin bearer token minted by the chart's bootstrap job so
	# kbs-client can authenticate against the KBS admin API.
	sudo mkdir -p "$(dirname "${KBS_ADMIN_TOKEN_FILE}")"
	kubectl get secret "${KBS_BOOTSTRAP_SECRET}" -n "${KBS_NS}" \
		-o jsonpath='{.data.KBS_ADMIN_TOKEN}' | \
		base64 -d | sudo tee "${KBS_ADMIN_TOKEN_FILE}" > /dev/null
	echo "::endgroup::"

	# Verify all three pods are running
	echo "::group::Verify pods are running"
	if ! waitForProcess "120" "10" \
		"kubectl -n ${KBS_NS} wait --for=condition=Ready pod -l app.kubernetes.io/instance=trustee --timeout=0 2>/dev/null"; then
		echo "ERROR: Not all Trustee pods are running"
		echo "::group::DEBUG - pods"
		kubectl -n "${KBS_NS}" get pods || true
		echo "::endgroup::"
		echo "::group::DEBUG - describe pods"
		kubectl -n "${KBS_NS}" describe pods || true
		echo "::endgroup::"
		echo "::group::DEBUG - kbs logs"
		kubectl -n "${KBS_NS}" logs -l app=kbs || true
		echo "::endgroup::"
		echo "::group::DEBUG - as logs"
		kubectl -n "${KBS_NS}" logs -l app=attestation-service || true
		echo "::endgroup::"
		echo "::group::DEBUG - rvps logs"
		kubectl -n "${KBS_NS}" logs -l app=reference-value-provider-service || true
		echo "::endgroup::"
		return 1
	fi
	echo "All Trustee pods are running"
	echo "::endgroup::"

	echo "::group::Post deploy actions"
	_post_deploy "${ingress}"
	echo "::endgroup::"

	# The KBS readiness probe hits /healthz, so a Ready pod (verified
	# above) already confirms the endpoint is working.

	if [[ -n "${ingress}" ]]; then
		echo "::group::Check the kbs service is exposed"
		svc_host=$(kbs_k8s_svc_http_addr)
		if [[ -z "${svc_host}" ]]; then
			echo "ERROR: service host not found"
			return 1
		fi

		# AZ DNS can take several minutes to update its records so that
		# the host name will take a while to start resolving.
		timeout=350
		echo "Trying to connect at ${svc_host}. Timeout=${timeout}"
		if ! waitForProcess "${timeout}" "30" "curl -s \"${svc_host}/healthz\" -o /dev/null -w '%{http_code}' | grep -q '200'"; then
			echo "ERROR: service seems to not respond on ${svc_host} host"
			curl -I "${svc_host}/healthz"
			return 1
		fi
		echo "KBS service respond to requests at ${svc_host}"
		echo "::endgroup::"
	fi
}

# Return the kbs service public IP in case ingress is configured
# otherwise the cluster IP.
#
kbs_k8s_svc_host() {
	if kubectl get ingress -n "${KBS_NS}" 2>/dev/null | grep -q kbs; then
		local host
		local timeout=50
		# The ingress IP address can take a while to show up.
		SECONDS=0
		while true; do
			host=$(kubectl get ingress "${KBS_INGRESS_NAME}" -n "${KBS_NS}" -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
			[[ -z "${host}" && ${SECONDS} -lt "${timeout}" ]] || break
			sleep 5
		done
		echo "${host}"
	elif kubectl get svc "${KBS_NODEPORT_SVC_NAME}" -n "${KBS_NS}" &>/dev/null; then
		local host
		host=$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}')
		echo "${host}"
	else
		kubectl get svc "${KBS_SVC_NAME}" -n "${KBS_NS}" \
			-o jsonpath='{.spec.clusterIP}' 2>/dev/null
	fi
}

# Return the kbs service port number. In case ingress is configured
# it will return "80", otherwise the pod's service port.
#
kbs_k8s_svc_port() {
	if kubectl get ingress -n "${KBS_NS}" 2>/dev/null | grep -q kbs; then
		# Assume served on default HTTP port 80
		echo "80"
	elif kubectl get svc "${KBS_NODEPORT_SVC_NAME}" -n "${KBS_NS}" &>/dev/null; then
		kubectl get svc "${KBS_NODEPORT_SVC_NAME}" -n "${KBS_NS}" -o jsonpath='{.spec.ports[0].nodePort}'
	else
		kubectl get svc "${KBS_SVC_NAME}" -n "${KBS_NS}" \
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

kbs_k8s_print_logs() {
	local start_time="$1"

	# Convert to iso time for kubectl
	local iso_start_time
	iso_start_time=$(date -d "${start_time}" --iso-8601=seconds)

	echo "::group::DEBUG - kbs logs since ${start_time}"
	kubectl -n "${KBS_NS}" logs -l app=kbs --since-time="${iso_start_time}" --timestamps=true || true
	echo "::endgroup::"
	echo "::group::DEBUG - as logs since ${start_time}"
	kubectl -n "${KBS_NS}" logs -l app=attestation-service --since-time="${iso_start_time}" --timestamps=true || true
	echo "::endgroup::"
	echo "::group::DEBUG - rvps logs since ${start_time}"
	kubectl -n "${KBS_NS}" logs -l app=reference-value-provider-service --since-time="${iso_start_time}" --timestamps=true || true
	echo "::endgroup::"
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
		source "${HOME}/.cargo/env"
	else
		[[ -z "${rust_version}" ]] && return

		# We don't want to mess with installation on bare-metal so
		# if rust is installed then just check it's >= the required
		# version.
		#
		local current_rust_version
		current_rust_version="$(rustc --version | cut -d' ' -f2)"
		if ! version_greater_than_equal "${current_rust_version}" \
			"${rust_version}"; then
			>&2 echo "ERROR: installed rust ${current_rust_version} < ${rust_version} (required)"
			return 1
		fi
	fi
}

# Run further actions after the kbs was deployed, usually to apply further
# configurations.
#
_post_deploy() {
	local ingress="${1:-}"

	if [[ "${ingress}" = "aks" ]]; then
		# The AKS managed ingress controller defaults to two nginx pod
		# replicas where both request 500m of CPU. On cluster made of small
		# VMs (e.g. 2 vCPU) one of the pod might not even start. We need only
		# one nginx, so patching the controller to keep only one replica.
		echo "Patch the ingress controller to have only one replica of nginx"
		waitForProcess "20" "5" \
			"kubectl patch nginxingresscontroller/default -n app-routing-system --type=merge -p='{\"spec\":{\"scaling\": {\"minReplicas\": 1}}}'"
	fi
}

# Prepare necessary resources for qemu-se runtime
# Documentation: https://github.com/confidential-containers/trustee/tree/main/attestation-service/verifier/src/se
prepare_credentials_for_qemu_se() {
	echo "::group::Prepare credentials for qemu-se runtime"
	if [[ -z "${IBM_SE_CREDS_DIR:-}" ]]; then
		>&2 echo "ERROR: IBM_SE_CREDS_DIR is empty"
		return 1
	fi
	config_file_path="/opt/kata/share/defaults/kata-containers/configuration-qemu-se.toml"
	kata_base_dir=$(dirname "$(kata-runtime --config "${config_file_path}" env --json | jq -r '.Kernel.Path')")
	if [[ -z "${HKD_PATH:-}" || ! -d "${HKD_PATH}" ]]; then
		>&2 echo "ERROR: HKD_PATH is not set"
		return 1
	fi
	pushd "${IBM_SE_CREDS_DIR}"
	mkdir {certs,crls,hdr,hkds,rsa}
	openssl genrsa -aes256 -passout pass:test1234 -out encrypt_key-psw.pem 4096
	openssl rsa -in encrypt_key-psw.pem -passin pass:test1234 -pubout -out rsa/encrypt_key.pub
	openssl rsa -in encrypt_key-psw.pem -passin pass:test1234 -out rsa/encrypt_key.pem
	cp "${kata_base_dir}/kata-containers-se.img" hdr/hdr.bin
	cp "${HKD_PATH}"/HKD-*.crt hkds/
	cp "${HKD_PATH}/ibm-z-host-key-gen2.crl" crls/
	cp "${HKD_PATH}/DigiCertCA.crt" "${HKD_PATH}/ibm-z-host-key-signing-gen2.crt" certs/
	popd
	ls -R "${IBM_SE_CREDS_DIR}"
	echo "::endgroup::"
}
