#!/bin/bash
#
# Copyright (c) 2021 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script is evoked within an OpenShift Build to product the binary image,
# which will contain the Kata Containers installation into a given destination
# directory.
#
# This contains variables and functions common to all e2e tests.

# Variables used by the kubernetes tests
export docker_images_nginx_version="1.15-alpine"
export container_images_agnhost_name="registry.k8s.io/e2e-test-images/agnhost"
export container_images_agnhost_version="2.21"

# Timeout options, mainly for use with waitForProcess(). Use them unless the
# operation needs to wait longer.
wait_time=90
sleep_time=3

# Timeout for use with `kubectl wait`, unless it needs to wait longer.
# Note: try to keep timeout and wait_time equal.
timeout=90s

# issues that can't test yet.
fc_limitations="https://github.com/kata-containers/documentation/issues/351"
dragonball_limitations="https://github.com/kata-containers/kata-containers/issues/6621"

# Path to the kubeconfig file which is used by kubectl and other tools.
# Note: the init script sets that variable but if you want to run the tests in
# your own provisioned cluster and you know what you are doing then you should
# overwrite it.
export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"

# ALLOW_ALL_POLICY is a Rego policy that allows all the Agent ttrpc requests.
K8S_TEST_DIR="${kubernetes_dir:-"${BATS_TEST_DIRNAME}"}"
ALLOW_ALL_POLICY="${ALLOW_ALL_POLICY:-$(base64 -w 0 "${K8S_TEST_DIR}/../../../src/kata-opa/allow-all.rego")}"

# Common setup for tests.
#
# Global variables exported:
#	$node	             - random picked node that has kata installed
#	$node_start_date     - start date/time at the $node for the sake of
#                          fetching logs
#
setup_common() {
	node=$(get_one_kata_node)
	[ -n "$node" ]
	node_start_time=$(exec_host "$node" date +\"%Y-%m-%d %H:%M:%S\")
	# If node_start_time is empty, try again 3 times with a 5 seconds sleep between each try.
	count=0
	while [ -z "$node_start_time" ] && [ $count -lt 3 ]; do
		echo "node_start_time is empty, trying again..."
		sleep 5
		node_start_time=$(exec_host "$node" date +\"%Y-%m-%d %H:%M:%S\")
		count=$((count + 1))
	done
	[ -n "$node_start_time" ]
	export node node_start_time

	k8s_delete_all_pods_if_any_exists || true
}

get_pod_config_dir() {
	pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads_work"
	info "k8s configured to use runtimeclass"
}

# Return the first worker found that is kata-runtime labeled.
get_one_kata_node() {
	local resource_name
	resource_name="$(kubectl get node -l katacontainers.io/kata-runtime=true -o name | head -1)"
	# Remove leading "/node"
	echo "${resource_name/"node/"}"
}

# Get the new debugger pod that wasn't present in the old_pods array.
get_new_debugger_pod() {
    local old_pods=("$@")
    local new_pod_list=($(kubectl get pods -o name | grep node-debugger))

    for new_pod in "${new_pod_list[@]}"; do
        if [[ ! " ${old_pods[*]} " =~ " ${new_pod} " ]]; then
            echo "${new_pod}"
            return
        fi
    done
}

# Runs a command in the host filesystem.
#
# Parameters:
#	$1 - the node name
#
exec_host() {
	local node="$1"
	# `kubectl debug` always returns 0, so we hack it to return the right exit code.
	local command="${@:2}"
	command+='; echo -en \\n$?'

	# Get the already existing debugger pods
	local old_debugger_pods=($(kubectl get pods -o name | grep node-debugger))

	# Run a debug pod
	kubectl debug -q "node/${node}" --image=quay.io/bedrock/ubuntu:latest -- chroot /host bash -c "sleep infinity" >&2

	# Identify the new debugger pod
	local new_debugger_pod=$(get_new_debugger_pod "${old_debugger_pods[@]}")

	# Wait for the newly created pod to be ready
	kubectl wait --timeout="30s" --for=condition=ready "${new_debugger_pod}" > /dev/null

	# Execute the command and capture the output
	# We're trailing the `\r` here due to: https://github.com/kata-containers/kata-containers/issues/8051
	# tl;dr: When testing with CRI-O we're facing the following error:
	# ```
	# (from function `exec_host' in file tests_common.sh, line 51,
	# in test file k8s-file-volume.bats, line 25)
	# `exec_host "echo "$file_body" > $tmp_file"' failed with status 127
	# [bats-exec-test:38] INFO: k8s configured to use runtimeclass
	# bash: line 1: $'\r': command not found
	# ```
	local output="$(kubectl exec -qi "${new_debugger_pod}" -- chroot /host bash -c "${command}" | tr -d '\r')"

	# Delete the newly created pod
	kubectl delete "${new_debugger_pod}" >&2

	# Output the command result
	local exit_code="$(echo "${output}" | tail -1)"
	echo "$(echo "${output}" | head -n -1)"
	return ${exit_code}
}

auto_generate_policy_enabled() {
	[ "${AUTO_GENERATE_POLICY}" == "yes" ]
}

# adapt common policy settings for tdx or snp
adapt_common_policy_settings_for_tdx() {
	local settings_dir=$1

	info "Adapting common policy settings for TDX or SNP"
	jq '.common.cpath = "/run/kata-containers" | .volumes.configMap.mount_point = "^$(cpath)/$(bundle-id)-[a-z0-9]{16}-"' "${settings_dir}/genpolicy-settings.json" > temp.json && sudo mv temp.json "${settings_dir}/genpolicy-settings.json"
}

# adapt common policy settings for qemu-sev
adapt_common_policy_settings_for_sev() {
	local settings_dir=$1

	info "Adapting common policy settings for SEV"
	jq '.kata_config.oci_version = "1.1.0-rc.1" | .common.cpath = "/run/kata-containers" | .volumes.configMap.mount_point = "^$(cpath)/$(bundle-id)-[a-z0-9]{16}-"' "${settings_dir}/genpolicy-settings.json" > temp.json && sudo mv temp.json "${settings_dir}/genpolicy-settings.json"
}

# adapt common policy settings for CBL-Mariner https://github.com/kata-containers/kata-containers/issues/10189
adapt_common_policy_settings_for_cbl_mariner() {
	local settings_dir=$1

	info "Adapting common policy settings for CBL-Mariner"
	jq '.request_defaults.UpdateEphemeralMountsRequest = true' "${settings_dir}/genpolicy-settings.json" > temp.json && sudo mv temp.json "${settings_dir}/genpolicy-settings.json"
	jq '.kata_config.oci_version = "1.1.0-rc.1"' "${settings_dir}/genpolicy-settings.json" > temp.json && sudo mv temp.json "${settings_dir}/genpolicy-settings.json"
}

# adapt common policy settings for various platforms
adapt_common_policy_settings() {

	local settings_dir=$1

	case "${KATA_HYPERVISOR}" in
  		"qemu-tdx"|"qemu-snp")
			adapt_common_policy_settings_for_tdx "${settings_dir}"
			;;
  		"qemu-sev")
			adapt_common_policy_settings_for_sev "${settings_dir}"
			;;
	esac

	case "${KATA_HOST_OS}" in
		"cbl-mariner")
			adapt_common_policy_settings_for_cbl_mariner "${settings_dir}"
			;;
	esac
}

# If auto-generated policy testing is enabled, make a copy of the genpolicy settings,
# and change these settings to use Kata CI cluster's default namespace.
create_common_genpolicy_settings() {
	declare -r genpolicy_settings_dir="$1"
	declare -r default_genpolicy_settings_dir="/opt/kata/share/defaults/kata-containers"

	auto_generate_policy_enabled || return 0

	adapt_common_policy_settings "${default_genpolicy_settings_dir}"

	cp "${default_genpolicy_settings_dir}/genpolicy-settings.json" "${genpolicy_settings_dir}"
	cp "${default_genpolicy_settings_dir}/rules.rego" "${genpolicy_settings_dir}"

	# Set the default namespace of Kata CI tests in the genpolicy settings.
	set_namespace_to_policy_settings "${genpolicy_settings_dir}" "${TEST_CLUSTER_NAMESPACE}"
}

# If auto-generated policy testing is enabled, make a copy of the common genpolicy settings
# described above into a temporary directory that will be used by the current test case.
create_tmp_policy_settings_dir() {
	declare -r common_settings_dir="$1"

	auto_generate_policy_enabled || return 0

	tmp_settings_dir=$(mktemp -d --tmpdir="${common_settings_dir}" genpolicy.XXXXXXXXXX)
	cp "${common_settings_dir}/rules.rego" "${tmp_settings_dir}"
	cp "${common_settings_dir}/genpolicy-settings.json" "${tmp_settings_dir}"

	echo "${tmp_settings_dir}"
}

# Delete a directory created by create_tmp_policy_settings_dir.
delete_tmp_policy_settings_dir() {
	local settings_dir="$1"

	auto_generate_policy_enabled || return 0

	if [ -d "${settings_dir}" ]; then
		info "Deleting ${settings_dir}"
		rm -rf "${settings_dir}"
	fi
}

# Execute genpolicy to auto-generate policy for a test YAML file.
auto_generate_policy() {
	declare -r settings_dir="$1"
	declare -r yaml_file="$2"
	declare -r config_map_yaml_file="$3"
	declare -r additional_flags="$4"

	auto_generate_policy_enabled || return 0
	local genpolicy_command="RUST_LOG=info /opt/kata/bin/genpolicy -u -y ${yaml_file}"
	genpolicy_command+=" -p ${settings_dir}/rules.rego"
	genpolicy_command+=" -j ${settings_dir}/genpolicy-settings.json"

	if [ ! -z "${config_map_yaml_file}" ]; then
		genpolicy_command+=" -c ${config_map_yaml_file}"
	fi

	if [ "${GENPOLICY_PULL_METHOD}" == "containerd" ]; then
		genpolicy_command+=" -d"
	fi

	genpolicy_command+=" ${additional_flags}"

	info "Executing: ${genpolicy_command}"
	eval "${genpolicy_command}"
}

# Change genpolicy settings to allow "kubectl exec" to execute a command
# and to read console output from a test pod.
add_exec_to_policy_settings() {
	auto_generate_policy_enabled || return 0

	local -r settings_dir="$1"
	shift

	# Create a JSON array of strings containing all the args of the command to be allowed.
	local exec_args=$(printf "%s\n" "$@" | jq -R | jq -sc)

	# Change genpolicy settings to allow kubectl to exec the command specified by the caller.
	local jq_command=".request_defaults.ExecProcessRequest.allowed_commands |= . + [${exec_args}]"
	info "${settings_dir}/genpolicy-settings.json: executing jq command: ${jq_command}"
	jq "${jq_command}" \
		"${settings_dir}/genpolicy-settings.json" > \
		"${settings_dir}/new-genpolicy-settings.json"
	mv "${settings_dir}/new-genpolicy-settings.json" \
		"${settings_dir}/genpolicy-settings.json"
}

# Change genpolicy settings to allow one or more ttrpc requests from the Host to the Guest.
add_requests_to_policy_settings() {
	declare -r settings_dir="$1"
	shift
	declare -r requests=("$@")

	auto_generate_policy_enabled || return 0

	for request in ${requests[@]}
	do
		info "${settings_dir}/genpolicy-settings.json: allowing ${request}"
		jq ".request_defaults.${request} |= true" \
			"${settings_dir}"/genpolicy-settings.json > \
			"${settings_dir}"/new-genpolicy-settings.json
		mv "${settings_dir}"/new-genpolicy-settings.json \
			"${settings_dir}"/genpolicy-settings.json
	done
}

# Change genpolicy settings to allow executing on the Guest VM the commands
# used by "kubectl cp" from the Host to the Guest.
add_copy_from_host_to_policy_settings() {
	local -r genpolicy_settings_dir="$1"

	local exec_command=(test -d /tmp)
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"
	exec_command=(tar -xmf - -C /tmp)
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"
}

# Change genpolicy settings to allow executing on the Guest VM the commands
# used by "kubectl cp" from the Guest to the Host.
add_copy_from_guest_to_policy_settings() {
	local -r genpolicy_settings_dir="$1"
	local -r copied_file="$2"

	exec_command=(tar cf - "${copied_file}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"
}

# Change genpolicy settings to use a pod namespace different than "default".
set_namespace_to_policy_settings() {
	local -r settings_dir="$1"
	local -r namespace="$2"

	auto_generate_policy_enabled || return 0

	info "${settings_dir}/genpolicy-settings.json: namespace: ${namespace}"
	jq --arg namespace "${namespace}" \
		'.cluster_config.default_namespace |= $namespace' \
		"${settings_dir}/genpolicy-settings.json" > \
		"${settings_dir}/new-genpolicy-settings.json"
	mv "${settings_dir}/new-genpolicy-settings.json" "${settings_dir}/genpolicy-settings.json"
}

hard_coded_policy_tests_enabled() {
	# CI is testing hard-coded policies just on a the platforms listed here. Outside of CI,
	# users can enable testing of the same policies (plus the auto-generated policies) by
	# specifying AUTO_GENERATE_POLICY=yes.
	local enabled_hypervisors="qemu-coco-dev qemu-sev qemu-snp qemu-tdx"
	[[ " $enabled_hypervisors " =~ " ${KATA_HYPERVISOR} " ]] || \
		[ "${KATA_HOST_OS}" == "cbl-mariner" ] || \
		auto_generate_policy_enabled
}

add_allow_all_policy_to_yaml() {
	hard_coded_policy_tests_enabled || return 0

	local yaml_file="$1"
	# Previous version of yq was not ready to handle multiple objects in a single yaml.
	# By default was changing only the first object.
	# With yq>4 we need to make it explicit during the read and write.
	local resource_kind="$(yq .kind ${yaml_file} | head -1)"

	case "${resource_kind}" in

	Pod)
		info "Adding allow all policy to ${resource_kind} from ${yaml_file}"
		ALLOW_ALL_POLICY="${ALLOW_ALL_POLICY}" yq -i \
			".metadata.annotations.\"io.katacontainers.config.agent.policy\" = \"${ALLOW_ALL_POLICY}\"" \
      "${yaml_file}"
		;;

	Deployment|Job|ReplicationController)
		info "Adding allow all policy to ${resource_kind} from ${yaml_file}"
		ALLOW_ALL_POLICY="${ALLOW_ALL_POLICY}" yq -i \
			".spec.template.metadata.annotations.\"io.katacontainers.config.agent.policy\" = \"${ALLOW_ALL_POLICY}\"" \
      "${yaml_file}"
		;;

	List)
		die "Issue #7765: adding allow all policy to ${resource_kind} from ${yaml_file} is not implemented yet"
		;;

	ConfigMap|LimitRange|Namespace|PersistentVolume|PersistentVolumeClaim|RuntimeClass|Secret|Service)
		die "Policy is not required for ${resource_kind} from ${yaml_file}"
		;;

	*)
		die "k8s resource type ${resource_kind} from ${yaml_file} is not yet supported for policy testing"
		;;

	esac
}

# Execute "kubectl describe ${pod}" in a loop, until its output contains "${endpoint} is blocked by policy"
wait_for_blocked_request() {
	local -r endpoint="$1"
	local -r pod="$2"

	local -r command="kubectl describe pod ${pod} | grep \"${endpoint} is blocked by policy\""
	info "Waiting ${wait_time} seconds for: ${command}"
	waitForProcess "${wait_time}" "$sleep_time" "${command}" >/dev/null 2>/dev/null
}

# Execute in a pod a command that is allowed by policy.
pod_exec_allowed_command() {
	local -r pod_name="$1"
	shift

	local -r exec_output=$(kubectl exec "${pod_name}" -- "${@}" 2>&1)

	local -r exec_args=$(printf '"%s",' "${@}")
	info "Pod ${pod_name}: <${exec_args::-1}>:"
	info "${exec_output}"

	(echo "${exec_output}" | grep "policy") && die "exec was blocked by policy!"
	return 0
}

# Execute in a pod a command that is blocked by policy.
pod_exec_blocked_command() {
	local -r pod_name="$1"
	shift

	local -r exec_output=$(kubectl exec "${pod_name}" -- "${@}" 2>&1)

	local -r exec_args=$(printf '"%s",' "${@}")
	info "Pod ${pod_name}: <${exec_args::-1}>:"
	info "${exec_output}"

	(echo "${exec_output}" | grep "ExecProcessRequest is blocked by policy" > /dev/null) || die "exec was not blocked by policy!"
}
