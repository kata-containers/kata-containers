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
export container_images_agnhost_name="registry.k8s.io/e2e-test-images/agnhost"
export container_images_agnhost_version="2.21"

# Timeout options, mainly for use with waitForProcess(). Use them unless the
# operation needs to wait longer.
export wait_time=90
export sleep_time=3

# Timeout for use with `kubectl wait`, unless it needs to wait longer.
# Note: try to keep timeout and wait_time equal.
export timeout=90s

# issues that can't test yet.
export fc_limitations="https://github.com/kata-containers/documentation/issues/351"
export dragonball_limitations="https://github.com/kata-containers/kata-containers/issues/6621"

# Path to the kubeconfig file which is used by kubectl and other tools.
# Note: the init script sets that variable but if you want to run the tests in
# your own provisioned cluster and you know what you are doing then you should
# overwrite it.
export KUBECONFIG="${KUBECONFIG:-${HOME}/.kube/config}"

K8S_TEST_DIR="${kubernetes_dir:-"${BATS_TEST_DIRNAME}"}"

AUTO_GENERATE_POLICY="${AUTO_GENERATE_POLICY:-}"
GENPOLICY_PULL_METHOD="${GENPOLICY_PULL_METHOD:-}"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-}"
KATA_HOST_OS="${KATA_HOST_OS:-}"

# Common setup for tests.
#
# Global variables exported:
#	$node	             - random picked node that has kata installed
#	$node_start_date     - start date/time at the $node for the sake of
#                          fetching logs
#
setup_common() {
	node=$(get_one_kata_node)
	[[ -n "${node}" ]]

	node_start_time=$(measure_node_time "${node}")

	export node node_start_time

	k8s_delete_all_pods_if_any_exists || true

	get_pod_config_dir
}

get_pod_config_dir() {
	export pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads_work"
	info "k8s configured to use runtimeclass"
}

# Return the first worker found that is kata-runtime labeled.
get_one_kata_node() {
	local resource_name
	resource_name="$(kubectl get node -l katacontainers.io/kata-runtime=true -o name | head -1)"
	# Remove leading "/node"
	echo "${resource_name/"node/"}"
}

auto_generate_policy_enabled() {
	[[ "${AUTO_GENERATE_POLICY}" == "yes" ]]
}

is_coco_platform() {
	case "${KATA_HYPERVISOR}" in
		"qemu-tdx"|"qemu-snp"|"qemu-coco-dev"|"qemu-coco-dev-runtime-rs"|"qemu-nvidia-gpu-tdx"|"qemu-nvidia-gpu-snp")
			return 0
			;;
		*)
			return 1
	esac
}

is_nvidia_gpu_platform() {
	case "${KATA_HYPERVISOR}" in
		qemu-nvidia-gpu*)
			return 0
			;;
		*)
			return 1
	esac
}

is_aks_cluster() {
	case "${KATA_HYPERVISOR}" in
		"qemu-tdx"|"qemu-snp"|qemu-nvidia-gpu*)
			return 1
			;;
		*)
			return 0
	esac
}

# Return the scenario suffix for genpolicy settings (e.g. "", "non-coco", "non-coco-aks").
# Used to select a pre-built settings file under share/defaults/kata-containers/.
get_genpolicy_settings_scenario() {
	if [[ "${KATA_HOST_OS}" == "cbl-mariner" ]]; then
		if is_coco_platform; then
			echo "cbl-mariner"
		elif is_aks_cluster; then
			echo "non-coco-aks-cbl-mariner"
		else
			echo "non-coco-cbl-mariner"
		fi
		return
	fi
	if is_nvidia_gpu_platform; then
		echo "nvidia-gpu"
		return
	fi
	if ! is_coco_platform; then
		if is_aks_cluster; then
			echo "non-coco-aks"
		else
			echo "non-coco"
		fi
		return
	fi
	echo ""
}

# If auto-generated policy testing is enabled, set up the genpolicy settings directory
# with base or scenario settings. Genpolicy is invoked with -j <dir> (see genpolicy README).
create_common_genpolicy_settings() {
	declare -r genpolicy_settings_dir="$1"
	declare -r default_genpolicy_settings_dir="/opt/kata/share/defaults/kata-containers"

	auto_generate_policy_enabled || return 0

	cp "${default_genpolicy_settings_dir}/rules.rego" "${genpolicy_settings_dir}"
	cp "${default_genpolicy_settings_dir}/genpolicy-settings.json" "${genpolicy_settings_dir}/genpolicy-settings.json"
	mkdir -p "${genpolicy_settings_dir}/genpolicy-settings.d"

	local scenario
	scenario="$(get_genpolicy_settings_scenario)"
	if [[ -n "${scenario}" ]] && [[ -f "${default_genpolicy_settings_dir}/drop-in-examples/10-${scenario}-drop-in.json" ]]; then
		info "Using genpolicy scenario drop-in: drop-in-examples/10-${scenario}-drop-in.json"
		cp "${default_genpolicy_settings_dir}/drop-in-examples/10-${scenario}-drop-in.json" "${genpolicy_settings_dir}/genpolicy-settings.d/"
	fi
}

# If auto-generated policy testing is enabled, make a copy of the common genpolicy settings
# directory (base + genpolicy-settings.d) for the current test case.
create_tmp_policy_settings_dir() {
	declare -r common_settings_dir="$1"

	auto_generate_policy_enabled || return 0

	tmp_settings_dir=$(mktemp -d --tmpdir="${common_settings_dir}" genpolicy.XXXXXXXXXX)
	cp "${common_settings_dir}/rules.rego" "${tmp_settings_dir}"
	cp "${common_settings_dir}/genpolicy-settings.json" "${tmp_settings_dir}"
	cp "${common_settings_dir}/default-initdata.toml" "${tmp_settings_dir}"
	if [[ -d "${common_settings_dir}/genpolicy-settings.d" ]]; then
		mkdir -p "${tmp_settings_dir}/genpolicy-settings.d"
		cp "${common_settings_dir}/genpolicy-settings.d/"*.json "${tmp_settings_dir}/genpolicy-settings.d/" 2>/dev/null || true
	fi

	echo "${tmp_settings_dir}"
}

# Delete a directory created by create_tmp_policy_settings_dir.
delete_tmp_policy_settings_dir() {
	local settings_dir="$1"

	auto_generate_policy_enabled || return 0

	if [[ -d "${settings_dir}" ]]; then
		info "Deleting ${settings_dir}"
		rm -rf "${settings_dir}"
	fi
}

# Execute genpolicy to auto-generate policy for a test YAML file.
auto_generate_policy() {
	declare -r settings_dir="$1"
	declare -r yaml_file="$2"
	declare -r config_map_yaml_file="${3:-""}"
	declare additional_flags="${4:-""}"

	additional_flags="${additional_flags} --initdata-path=${settings_dir}/default-initdata.toml"

	auto_generate_policy_no_added_flags "${settings_dir}" "${yaml_file}" "${config_map_yaml_file}" "${additional_flags}"
}

auto_generate_policy_no_added_flags() {
	declare -r settings_dir="$1"
	declare -r yaml_file="$2"
	declare -r config_map_yaml_file="${3:-""}"
	declare -r additional_flags="${4:-""}"

	auto_generate_policy_enabled || return 0
	local genpolicy_command="RUST_LOG=info /opt/kata/bin/genpolicy -u -y ${yaml_file}"
	genpolicy_command+=" -p ${settings_dir}/rules.rego"
	genpolicy_command+=" -j ${settings_dir}"

	if [[ -n "${config_map_yaml_file}" ]]; then
		genpolicy_command+=" -c ${config_map_yaml_file}"
	fi

	if [[ "${GENPOLICY_PULL_METHOD}" == "containerd" ]]; then
		genpolicy_command+=" -d"
	fi

	genpolicy_command+=" ${additional_flags}"

	# Retry if genpolicy fails, because typical failures of this tool are caused by
	# transient network errors.
	for _ in {1..6}; do
		info "Executing: ${genpolicy_command}"
		eval "${genpolicy_command}" && return 0
		info "Sleeping after command failed..."
		sleep 10s
	done
	return 1
}

# Resolve drop-in-examples source directory (installed path only). Dies if missing.
get_genpolicy_drop_in_examples_dir() {
	local defaults_dir="/opt/kata/share/defaults/kata-containers"
	if [[ ! -d "${defaults_dir}/drop-in-examples" ]]; then
		die "drop-in-examples not found: ${defaults_dir}/drop-in-examples"
	fi
	echo "${defaults_dir}/drop-in-examples"
}

# Change genpolicy settings to allow "kubectl exec" to execute a command
# and to read console output from a test pod. Writes to genpolicy-settings.d/99-exec.json (drop-in).
# Copies from the shipped drop-in-examples/99-exec.json.
add_exec_to_policy_settings() {
	auto_generate_policy_enabled || return 0

	local -r settings_dir="$1"
	shift

	mkdir -p "${settings_dir}/genpolicy-settings.d"
	local dropin="${settings_dir}/genpolicy-settings.d/99-exec.json"
	local drop_in_examples_dir
	drop_in_examples_dir="$(get_genpolicy_drop_in_examples_dir)"
	local src="${drop_in_examples_dir}/99-exec.json"
	if [[ ! -f "${src}" ]]; then
		die "Missing drop-in ${src} under ${drop_in_examples_dir}/"
	fi
	if [[ ! -f "${dropin}" ]]; then
		cp "${src}" "${dropin}"
	fi

	local exec_args
	exec_args=$(printf "%s\n" "$@" | jq -R | jq -sc)
	local jq_command=".request_defaults.ExecProcessRequest.allowed_commands = ((.request_defaults.ExecProcessRequest.allowed_commands // []) + [${exec_args}])"
	info "${dropin}: adding allowed_commands"
	jq "${jq_command}" "${dropin}" > "${dropin}.tmp" && mv "${dropin}.tmp" "${dropin}"
}

# Change genpolicy settings to allow one or more ttrpc requests from the Host to the Guest.
# Writes one drop-in per request: genpolicy-settings.d/99-<RequestName>.json.
# Copies from the shipped drop-in-examples/.
add_requests_to_policy_settings() {
	declare -r settings_dir="$1"
	shift
	declare -r requests=("$@")

	auto_generate_policy_enabled || return 0

	mkdir -p "${settings_dir}/genpolicy-settings.d"
	local drop_in_examples_dir
	drop_in_examples_dir="$(get_genpolicy_drop_in_examples_dir)"

	for request in "${requests[@]}"
	do
		local src="${drop_in_examples_dir}/99-${request}.json"
		local dropin="${settings_dir}/genpolicy-settings.d/99-${request}.json"
		if [[ ! -f "${src}" ]]; then
			die "Missing drop-in ${src} under ${drop_in_examples_dir}/"
		fi
		info "${dropin}: copying from drop-in-examples"
		cp "${src}" "${dropin}"
	done
}

# Change genpolicy settings to allow executing on the Guest VM the commands
# used by "kubectl cp" from the Host to the Guest.
add_copy_from_host_to_policy_settings() {
	local -r genpolicy_settings_dir="$1"

	local exec_command=(test -d /tmp)
	add_exec_to_policy_settings "${genpolicy_settings_dir}" "${exec_command[@]}"
	exec_command=(tar -xmf - -C /tmp)
	add_exec_to_policy_settings "${genpolicy_settings_dir}" "${exec_command[@]}"
}

# Change genpolicy settings to allow executing on the Guest VM the commands
# used by "kubectl cp" from the Guest to the Host.
add_copy_from_guest_to_policy_settings() {
	local -r genpolicy_settings_dir="$1"
	local -r copied_file="$2"

	exec_command=(tar cf - "${copied_file}")
	add_exec_to_policy_settings "${genpolicy_settings_dir}" "${exec_command[@]}"
}

hard_coded_policy_tests_enabled() {
	local enabled="no"
	# CI is testing hard-coded policies just on a the platforms listed here. Outside of CI,
	# users can enable testing of the same policies (plus the auto-generated policies) by
	# specifying AUTO_GENERATE_POLICY=yes.
	local -r enabled_hypervisors=("qemu-coco-dev" "qemu-snp" "qemu-tdx" "qemu-coco-dev-runtime-rs")
	for enabled_hypervisor in "${enabled_hypervisors[@]}"
	do
		if [[ "${enabled_hypervisor}" == "${KATA_HYPERVISOR}" ]]; then
			enabled="yes"
			break
		fi
	done

	if [[ "${enabled}" == "no" && "${KATA_HOST_OS}" == "cbl-mariner" ]]; then
		enabled="yes"
	fi

	if [[ "${enabled}" == "no" ]] && auto_generate_policy_enabled; then
		enabled="yes"
	fi

	[[ "${enabled}" == "yes" ]]
}

encode_policy_in_init_data() {
  local input="$1"   # either a filename or a policy
  local POLICY

  # if input is a file, read its contents
  if [[ -f "${input}" ]]; then
    POLICY="$(< "${input}")"
  else
    POLICY="${input}"
  fi

  cat <<EOF | gzip -c | base64 -w0
version = "0.1.0"
algorithm = "sha256"

[data]
"policy.rego" = '''
${POLICY}
'''
EOF
}

# ALLOW_ALL_POLICY is a Rego policy that allows all the Agent ttrpc requests.
ALLOW_ALL_POLICY="${ALLOW_ALL_POLICY:-$(encode_policy_in_init_data "${K8S_TEST_DIR}/../../../src/kata-opa/allow-all.rego")}"

add_allow_all_policy_to_yaml() {
	hard_coded_policy_tests_enabled || return 0

	local yaml_file="$1"
	# Previous version of yq was not ready to handle multiple objects in a single yaml.
	# By default was changing only the first object.
	# With yq>4 we need to make it explicit during the read and write.
	local resource_kind
	resource_kind=$(yq eval 'select(documentIndex == 0) | .kind' "${yaml_file}")

	case "${resource_kind}" in
	Pod)
		info "Adding allow all policy to ${resource_kind} from ${yaml_file}"
		yq -i \
			".metadata.annotations.\"io.katacontainers.config.hypervisor.cc_init_data\" = \"${ALLOW_ALL_POLICY}\"" \
      "${yaml_file}"
		;;

	Deployment|Job|ReplicationController)
		info "Adding allow all policy to ${resource_kind} from ${yaml_file}"
		yq -i \
			".spec.template.metadata.annotations.\"io.katacontainers.config.hypervisor.cc_init_data\" = \"${ALLOW_ALL_POLICY}\"" \
      "${yaml_file}"
		;;

	List)
		die "Issue #7765: adding allow all policy to ${resource_kind} from ${yaml_file} is not implemented yet"
		;;

	ConfigMap|LimitRange|Namespace|PersistentVolume|PersistentVolumeClaim|RuntimeClass|Secret|Service)
		info "Policy is not required for ${resource_kind} from ${yaml_file}"
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
	waitForProcess "${wait_time}" "${sleep_time}" "${command}" >/dev/null 2>/dev/null
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

# Common teardown for tests.
#
# Parameters:
#	$1	- node name where kata is installed
#	$2	- start time at the node for the sake of fetching logs
#
teardown_common() {
	local node="$1"
	local node_start_time="$2"

	kubectl describe pods
	k8s_delete_all_pods_if_any_exists || true

	local node_end_time
	node_end_time=$(measure_node_time "${node}")

	echo "Journal LOG starts at ${node_start_time:-}, ends at ${node_end_time:-}"

	# Print the node journal since the test start time if a bats test is not completed
	if [[ -n "${node_start_time}" && -z "${BATS_TEST_COMPLETED}" ]]; then
		echo "DEBUG: system logs of node '${node}' since test start time (${node_start_time})"
		exec_host "${node}" journalctl -x -t "kata" --since '"'"${node_start_time}"'"' || true
	fi
}

measure_node_time() {
	local node="$1"
	[[ -n "${node}" ]]

	local node_time
	node_time=$(exec_host "${node}" date +\"%Y-%m-%d %H:%M:%S\")
	local count=0
	while [[ -z "${node_time}" ]] && [[ "${count}" -lt 3 ]]; do
		echo "node_time is empty, trying again..."
		sleep 2
		node_time=$(exec_host "${node}" date +\"%Y-%m-%d %H:%M:%S\")
		count=$((count + 1))
	done
	[[ -n "${node_time}" ]]

	printf '%s\n' "${node_time}"
}

# Execute a command in a pod and grep kubectl's output.
#
# Parameters:
#	$1	- pod name
#	$2	- the grep pattern
#	$3+	- the command to execute using "kubectl exec"
#
# Exit code:
#	Equal to grep's exit code
grep_pod_exec_output() {
	local -r pod_name="$1"
	shift
	local -r grep_arg="$1"
	shift
	pod_exec "${pod_name}" "$@" | grep "${grep_arg}"
}

# Execute a command in a pod and echo kubectl's output to stdout.
#
# Parameters:
#	$1	- pod name
#	$2+	- the command to execute using "kubectl exec"
#
# Exit code:
#	0
pod_exec() {
	local -r pod_name="$1"
	shift
	local -r container_name=""

	container_exec "${pod_name}" "${container_name}" "$@"
}

# Execute a command in a pod's container and echo kubectl's output to stdout.
#
# If the caller specifies an empty container name as parameter, the command is executed in pod's default container,
# or in pod's first container if there is no default.
#
# Parameters:
#	$1	- pod name
#	$2	- container name
#	$3+	- the command to execute using "kubectl exec"
#
# Exit code:
#	0
container_exec() {
	local -r pod_name="$1"
	shift
	local -r container_name="$1"
	shift
	local cmd_out=""

	if [[ -n "${container_name}" ]]; then
		bats_unbuffered_info "Executing in pod ${pod_name}, container ${container_name}: $*"
		if ! cmd_out=$(kubectl exec "${pod_name}" -c "${container_name}" -- "$@"); then
			bats_unbuffered_info "kubectl exec failed"
			cmd_out=""
			# preserve failure semantics: return kubectl's exit code
			return 1
		fi
	else
		bats_unbuffered_info "Executing in pod ${pod_name}: $*"
		if ! cmd_out=$(kubectl exec "${pod_name}" -- "$@"); then
			bats_unbuffered_info "kubectl exec failed"
			cmd_out=""
			# preserve failure semantics: return kubectl's exit code
			return 1
		fi
	fi

	if [[ -n "${cmd_out}" ]]; then
		bats_unbuffered_info "command output: ${cmd_out}"
	else
		bats_unbuffered_info "Warning: empty output from kubectl exec"
	fi

	echo "${cmd_out}"
}

set_nginx_image() {
	input_yaml=$1
	output_yaml=$2

	ensure_yq
	nginx_registry=$(get_from_kata_deps ".docker_images.nginx.registry")
	nginx_digest=$(get_from_kata_deps ".docker_images.nginx.digest")
	nginx_image="${nginx_registry}@${nginx_digest}"

	NGINX_IMAGE="${nginx_image}" envsubst < "${input_yaml}" > "${output_yaml}"
}

print_node_journal_since_test_start() {
	local node="${1}"
	local node_start_time="${2:-}"
	local BATS_TEST_COMPLETED="${3:-}"

	if [[ -n "${node_start_time:-}" && -z "${BATS_TEST_COMPLETED:-}" ]]; then
		echo "DEBUG: system logs of node '${node}' since test start time (${node_start_time})"
		exec_host "${node}" journalctl -x -t "kata" --since '"'"${node_start_time}"'"' || true
	fi
}
