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

# Runs a command in the host filesystem.
#
# Parameters:
#	$1 - the node name
#
exec_host() {
	node="$1"
	# `kubectl debug` always returns 0, so we hack it to return the right exit code.
	command="${@:2}"
	command+='; echo -en \\n$?'
	# We're trailing the `\r` here due to: https://github.com/kata-containers/kata-containers/issues/8051
	# tl;dr: When testing with CRI-O we're facing the foillowing error:
	# ```
	# (from function `exec_host' in file tests_common.sh, line 51,
	# in test file k8s-file-volume.bats, line 25)
	# `exec_host "echo "$file_body" > $tmp_file"' failed with status 127
	# [bats-exec-test:38] INFO: k8s configured to use runtimeclass
	# bash: line 1: $'\r': command not found
	# ```
	output="$(kubectl debug -qit "node/${node}" --image=alpine:latest -- chroot /host bash -c "${command}" | tr -d '\r')"
	kubectl get pods -o name | grep node-debugger | xargs kubectl delete > /dev/null
	exit_code="$(echo "${output}" | tail -1)"
	echo "$(echo "${output}" | head -n -1)"
	return ${exit_code}
}

auto_generate_policy_enabled() {
	[ "${AUTO_GENERATE_POLICY}" == "yes" ]
}

# If auto-generated policy testing is enabled, make a copy of the genpolicy settings,
# and change these settings to use Kata CI cluster's default namespace.
create_common_genpolicy_settings() {
	declare -r genpolicy_settings_dir="$1"
	declare -r default_genpolicy_settings_dir="/opt/kata/share/defaults/kata-containers"

	auto_generate_policy_enabled || return 0

	cp "${default_genpolicy_settings_dir}/genpolicy-settings.json" "${genpolicy_settings_dir}"
	cp "${default_genpolicy_settings_dir}/rules.rego" "${genpolicy_settings_dir}"

	# Set the default namespace of Kata CI tests in the genpolicy settings.
	info "${genpolicy_settings_dir}/genpolicy-settings.json: default namespace: ${test_cluster_namespace}"
	jq --arg test_cluster_namespace "${test_cluster_namespace}" \
		'.cluster_config.default_namespace |= $test_cluster_namespace' \
		"${genpolicy_settings_dir}/genpolicy-settings.json" > \
		"${genpolicy_settings_dir}/new-genpolicy-settings.json"
	mv "${genpolicy_settings_dir}/new-genpolicy-settings.json" "${genpolicy_settings_dir}/genpolicy-settings.json"
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

	auto_generate_policy_enabled || return 0

	local genpolicy_command="RUST_LOG=info /opt/kata/bin/genpolicy -u -y ${yaml_file}"
	genpolicy_command+=" -p ${settings_dir}/rules.rego"
	genpolicy_command+=" -j ${settings_dir}/genpolicy-settings.json"

	if [ ! -z "${config_map_yaml_file}" ]; then
		genpolicy_command+=" -c ${config_map_yaml_file}"
	fi

	info "Executing: ${genpolicy_command}"
	eval "${genpolicy_command}"
}

# Change genpolicy settings to allow "kubectl exec" to execute a command
# and to read console output from a test pod.
add_exec_to_policy_settings() {
	declare -r settings_dir="$1"
	declare -r allowed_exec="$2"

	auto_generate_policy_enabled || return 0

	# Change genpolicy settings to allow kubectl to exec the command specified by the caller.
	info "${settings_dir}/genpolicy-settings.json: allowing exec: ${allowed_exec}"
	jq --arg allowed_exec "${allowed_exec}" \
		'.request_defaults.ExecProcessRequest.commands |= . + [$allowed_exec]' \
		"${settings_dir}/genpolicy-settings.json" > \
		"${settings_dir}/new-genpolicy-settings.json"
	mv "${settings_dir}/new-genpolicy-settings.json" \
		"${settings_dir}/genpolicy-settings.json"

	# Change genpolicy settings to allow kubectl to read the output of the command being executed.
	info "${settings_dir}/genpolicy-settings.json: allowing ReadStreamRequest"
	jq '.request_defaults.ReadStreamRequest |= true' \
		"${settings_dir}"/genpolicy-settings.json > \
		"${settings_dir}"/new-genpolicy-settings.json
	mv "${settings_dir}"/new-genpolicy-settings.json \
		"${settings_dir}"/genpolicy-settings.json
}
