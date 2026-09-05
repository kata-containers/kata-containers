#!/bin/bash
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Shared helm deployment helpers for kata-deploy tests
#
# Required environment variables:
#   DOCKER_REGISTRY - Container registry for kata-deploy image
#   DOCKER_REPO     - Repository name for kata-deploy image
#   DOCKER_TAG      - Image tag to test
#   KATA_HYPERVISOR - Hypervisor to test (qemu, clh, etc.)
#   KUBERNETES      - K8s distribution (microk8s, k3s, rke2, etc.)
# Optional EROFS settings:
#   SNAPSHOTTER                 - Set to "erofs" to configure the EROFS snapshotter
#   EROFS_SNAPSHOTTER_MODE      - "disk" or "memory"
#   EROFS_DMVERITY              - Set to "dmverity" to enable dm-verity
#   EROFS_MERGE_MODE            - "merged" or "unmerged"
#   EROFS_UTILS_IMAGE           - Image kata-deploy takes erofs-utils from

HELM_RELEASE_NAME="${HELM_RELEASE_NAME:-kata-deploy}"
HELM_NAMESPACE="${HELM_NAMESPACE:-kube-system}"

# Run a command against the host node's filesystem, mounted at /host inside a
# short-lived privileged pod.
# Usage: run_on_host "test -d /host/opt/kata && echo YES || echo NO"
#
# We avoid `kubectl run --rm -i` because rke2 injects session-recording banners
# into interactive pods, polluting stdout. Instead: create, wait, fetch logs, delete.
run_on_host() {
	local cmd="$1"
	local node_name
	node_name=$(kubectl get nodes --no-headers -o custom-columns=NAME:.metadata.name | head -1)
	local pod_name="host-exec-${RANDOM}"

	kubectl run "${pod_name}" \
		--image=quay.io/kata-containers/alpine-bash-curl:latest \
		--restart=Never \
		--overrides="{
			\"spec\": {
				\"nodeName\": \"${node_name}\",
				\"activeDeadlineSeconds\": 300,
				\"tolerations\": [{\"operator\": \"Exists\"}],
				\"containers\": [{
					\"name\": \"exec\",
					\"image\": \"quay.io/kata-containers/alpine-bash-curl:latest\",
					\"imagePullPolicy\": \"IfNotPresent\",
					\"command\": [\"sh\", \"-c\", \"${cmd}\"],
					\"securityContext\": {\"privileged\": true},
					\"volumeMounts\": [{\"name\": \"host\", \"mountPath\": \"/host\", \"readOnly\": true}]
				}],
				\"volumes\": [{\"name\": \"host\", \"hostPath\": {\"path\": \"/\"}}]
			}
		}" > /dev/null 2>&1

	local deadline=$((SECONDS + 60))
	while (( SECONDS < deadline )); do
		local phase
		phase=$(kubectl get pod "${pod_name}" -o jsonpath='{.status.phase}' 2>/dev/null) || true
		case "${phase}" in
			Succeeded|Failed) break ;;
		esac
		sleep 1
	done

	kubectl logs "${pod_name}" 2>/dev/null
	kubectl delete pod "${pod_name}" --ignore-not-found=true > /dev/null 2>&1
	[[ "${phase}" == "Succeeded" ]]
}

# The tolerations of a rendered pod spec, one entry per line, its fields joined
# by "; " in the order they were rendered.
#
# Grepping for a single field cannot say which entry that field belongs to, and
# the difference matters here: a bare `operator: Exists` tolerates every taint,
# while the same operator next to a key tolerates exactly one.
tolerations_of() {
	awk '
		!inside && /^[[:space:]]*tolerations:[[:space:]]*$/ {
			match($0, /^[[:space:]]*/)
			indent = RLENGTH
			inside = 1
			next
		}
		inside && /^[[:space:]]*$/ { next }
		inside {
			match($0, /^[[:space:]]*/)
			if (RLENGTH <= indent) {
				if (entry != "") { print entry; entry = "" }
				inside = 0
				next
			}
			field = substr($0, RLENGTH + 1)
			if (sub(/^- /, "", field)) {
				if (entry != "") print entry
				entry = field
			} else {
				entry = entry "; " field
			}
		}
		END { if (entry != "") print entry }
	'
}

# Whether the release installed a kata-deploy DaemonSet, i.e. whether it is in
# daemonset mode. Callers that take the chart default cannot know the mode, and
# the DaemonSet pods and the per-node Job pods share no label.
kata_deploy_ds_exists() {
	[[ -n "$(kubectl -n "${HELM_NAMESPACE}" get ds -l name=kata-deploy -o name 2>/dev/null)" ]]
}

# The label selector matching whichever pods run the install.
kata_deploy_pod_selector() {
	if kata_deploy_ds_exists; then
		echo "name=kata-deploy"
	else
		echo "app.kubernetes.io/name=kata-deploy"
	fi
}

# Get the path to the helm chart
get_chart_path() {
	local script_dir
	script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
	echo "${script_dir}/../../../../tools/packaging/kata-deploy/helm-chart/kata-deploy"
}

# The values file defining a shim, when values.yaml does not.
#
# values.yaml carries the generic shims only; the ones needing a snapshotter set
# up or hardware on the node ship as try-kata-*.values.yaml profiles. Enabling
# such a shim by name is not enough to install it - the block also has to say
# which architectures it supports, and the rest of what the shim needs is in the
# profile too - so the tests deploy the profile and override what they care
# about, rather than describing the shim themselves.
#
# Prints nothing when values.yaml already defines the shim.
# Arguments:
#   $1 - Shim name
shim_profile_file() {
	local shim="$1"
	local chart_path profile

	chart_path="$(get_chart_path)"
	if [[ "$(yq ".shims | has(\"${shim}\")" "${chart_path}/values.yaml")" == "true" ]]; then
		return 0
	fi

	for profile in "${chart_path}"/try-kata-*.values.yaml; do
		[[ -f "${profile}" ]] || continue
		if [[ "$(yq ".shims | has(\"${shim}\")" "${profile}")" == "true" ]]; then
			echo "${profile}"
			return 0
		fi
	done
}

# The shims a profile enables besides the one under test, one per line.
#
# A profile turns on every shim it ships, and an explicit enabled: true outlives
# the disableAll below, so the ones not under test have to be named and turned
# off for the install to stay the single-shim one the tests expect.
# Arguments:
#   $1 - Shim name
profile_shims_to_disable() {
	local shim="$1"
	local profile

	profile="$(shim_profile_file "${shim}")"
	[[ -n "${profile}" ]] || return 0

	yq ".shims | to_entries | map(select(.value.enabled == true and .key != \"${shim}\") | .key) | .[]" \
		"${profile}"
}

# Generate base values YAML that disables all shims except the specified one
# Arguments:
#   $1 - Output file path
#   $2 - (Optional) Additional values file to merge
# shellcheck disable=SC2154
generate_base_values() {
	local output_file="$1"
	local extra_values_file="${2:-}"

	local k8s_distribution="${KUBERNETES}"
	if [[ "${k8s_distribution}" == "kubeadm" ]]; then
		k8s_distribution="k8s"
	fi

	local shim_snapshotter_values=""
	local snapshotter_values=""
	if [[ "${SNAPSHOTTER:-}" == "erofs" ]]; then
		local erofs_dmverity="false"
		if [[ "${EROFS_DMVERITY:-}" == "dmverity" ]]; then
			erofs_dmverity="true"
		fi

		shim_snapshotter_values="    containerd:
      snapshotter: erofs"
		snapshotter_values="snapshotter:
  setup: [\"erofs\"]
  erofsSnapshotterMode: \"${EROFS_SNAPSHOTTER_MODE:-}\"
  erofsDmverity: ${erofs_dmverity}
  erofsMergeMode: \"${EROFS_MERGE_MODE:-}\"

# The node has no erofs-utils of its own; see deploy_k8s.
nodeBinaries:
  erofs-utils:
    image: \"${EROFS_UTILS_IMAGE:-quay.io/kata-containers/erofs-utils:1.9.3}\"
    binaries: [mkfs.erofs]

# fs-verity would also need the backing filesystem prepared for it, and these
# tests only care about dm-verity.
containerd:
  userDropIn: |
    [plugins.'io.containerd.snapshotter.v1.erofs']
      enable_fsverity = false"
	fi

	local other_shim_values=""
	local other_shim
	while read -r other_shim; do
		[[ -n "${other_shim}" ]] || continue
		other_shim_values+="  ${other_shim}:
    enabled: false
"
	done < <(profile_shims_to_disable "${KATA_HYPERVISOR}")

	cat > "${output_file}" <<EOF
image:
  reference: ${DOCKER_REGISTRY}/${DOCKER_REPO}
  tag: ${DOCKER_TAG}

k8sDistribution: "${k8s_distribution}"
debug: true

# Disable all shims at once, then enable only the one we need. A shim coming
# from a profile is already enabled there, along with the shims it ships next to,
# which are the ones named below.
shims:
  disableAll: true
  ${KATA_HYPERVISOR}:
    enabled: true
${shim_snapshotter_values}
${other_shim_values}

defaultShim:
  amd64: ${KATA_HYPERVISOR}
  arm64: ${KATA_HYPERVISOR}

runtimeClasses:
  enabled: true
  createDefault: true

${snapshotter_values}
EOF
}

# Deploy kata-deploy using helm
# Arguments:
#   $1 - (Optional) Additional values file to merge with base values
#   $@ - (Optional) Additional helm arguments (after the first positional arg)
deploy_kata() {
	local extra_values_file="${1:-}"
	shift || true
	local extra_helm_args=("$@")

	local chart_path
	local values_yaml
	local profile_values

	chart_path="$(get_chart_path)"
	values_yaml=$(mktemp)

	# Generate base values
	generate_base_values "${values_yaml}"

	# The profile defining the shim under test, if values.yaml does not, first so
	# that the generated values override it.
	profile_values="$(shim_profile_file "${KATA_HYPERVISOR}")"

	# NFD is vendored under charts/*.tgz; no helm dependency fetch needed.

	# Build helm command
	local helm_cmd=(
		helm upgrade --install "${HELM_RELEASE_NAME}" "${chart_path}"
	)

	if [[ -n "${profile_values}" ]]; then
		helm_cmd+=(-f "${profile_values}")
	fi

	helm_cmd+=(-f "${values_yaml}")

	# Add extra values file if provided
	if [[ -n "${extra_values_file}" && -f "${extra_values_file}" ]]; then
		helm_cmd+=(-f "${extra_values_file}")
	fi

	# Add any extra helm arguments
	if [[ ${#extra_helm_args[@]} -gt 0 ]]; then
		helm_cmd+=("${extra_helm_args[@]}")
	fi

	helm_cmd+=(
		--namespace "${HELM_NAMESPACE}"
		--wait --timeout "${HELM_TIMEOUT:-10m}"
	)

	# The install is complete once helm returns: --wait blocks on DaemonSet
	# readiness, whose probe only passes after install, and hooks always block -
	# the job-mode dispatcher is one, and it waits for every per-node Job.
	"${helm_cmd[@]}"
	local ret=$?

	rm -f "${values_yaml}"

	if [[ ${ret} -ne 0 ]]; then
		echo "Helm install failed with exit code ${ret}" >&2
		return "${ret}"
	fi

	# helm --wait can call a single-node maxUnavailable=1 DaemonSet ready with 0
	# ready pods.
	if kata_deploy_ds_exists; then
		kubectl -n "${HELM_NAMESPACE}" wait pod -l name=kata-deploy \
			--for=condition=Ready --timeout="${HELM_TIMEOUT:-10m}" 2>/dev/null || true
	fi

	return 0
}

# Uninstall kata-deploy
uninstall_kata() {
	helm uninstall "${HELM_RELEASE_NAME}" -n "${HELM_NAMESPACE}" \
		--ignore-not-found --wait --cascade foreground --timeout 10m || true

	wait_for_api_and_retry_uninstall "${HELM_RELEASE_NAME}" "${HELM_NAMESPACE}"
}
