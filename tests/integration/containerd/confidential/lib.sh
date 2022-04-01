#!/bin/bash
# Copyright (c) 2021, 2022 IBM Corporation
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This provides generic functions to use in the tests.
#
set -e

source "${BATS_TEST_DIRNAME}/../../../lib/common.bash"
FIXTURES_DIR="${BATS_TEST_DIRNAME}/fixtures"

# Delete the containers alongside the Pod.
#
# Parameters:
#	$1 - the sandbox name
#
crictl_delete_cc_pod() {
	local sandbox_name="$1"
	local pod_id="$(sudo crictl pods --name ${sandbox_name} -q)"
	local container_ids="$(sudo crictl ps --pod ${pod_id} -q)"

	if [ -n "${container_ids}" ]; then
		while read -r container_id; do
			sudo crictl stop "${container_id}"
			sudo crictl rm "${container_id}"
		done <<< "${container_ids}"
	fi
	sudo crictl stopp "${pod_id}"
	sudo crictl rmp "${pod_id}"
}

# Delete the pod if it exists, otherwise just return.
#
# Parameters:
#	$1 - the sandbox name
#
crictl_delete_cc_pod_if_exists() {
	local sandbox_name="$1"

	[ -z "$(sudo crictl pods --name ${sandbox_name} -q)" ] || \
		crictl_delete_cc_pod "${sandbox_name}"
}

# Wait until the pod is not 'Ready'. Fail if it hits the timeout.
#
# Parameters:
#	$1 - the sandbox ID
#	$2 - wait time in seconds. Defaults to 10. (optional)
#	$3 - sleep time in seconds between checks. Defaults to 5. (optional)
#
crictl_wait_cc_pod_be_ready() {
	local pod_id="$1"
	local wait_time="${2:-10}"
	local sleep_time="${3:-5}"

	local cmd="[ \$(sudo crictl pods --id $pod_id -q --state ready |\
	       	wc -l) -eq 1 ]"
	if ! waitForProcess "$wait_time" "$sleep_time" "$cmd"; then
		echo "Pod ${pod_id} not ready after ${wait_time}s"
		return 1
	fi
}

# Create a pod and wait it be ready, otherwise fail.
#
# Parameters:
#	$1 - the pod configuration file.
#
crictl_create_cc_pod() {
	local config_file="$1"
	local pod_id=""

	if [ ! -f "$config_file" ]; then
		echo "Pod config file '${config_file}' does not exist"
		return 1
	fi

	if ! pod_id=$(sudo crictl runp -r kata "$config_file"); then
		echo "Failed to create the pod"
		return 1
	fi

	if ! crictl_wait_cc_pod_be_ready "$pod_id"; then
		# TODO: run this command for debugging. Maybe it should be
		#       guarded by DEBUG=true?
		sudo crictl pods
		return 1
	fi
}

# Wait until the container does not start running. Fail if it hits the timeout.
#
# Parameters:
#	$1 - the container ID.
#	$2 - wait time in seconds. Defaults to 30. (optional)
#	$3 - sleep time in seconds between checks. Defaults to 10. (optional)
#
crictl_wait_cc_container_be_running() {
	local container_id="$1"
	local wait_time="${2:-30}"
	local sleep_time="${3:-10}"

	local cmd="[ \$(sudo crictl ps --id $container_id -q --state running | \
		wc -l) -eq 1 ]"
	if ! waitForProcess "$wait_time" "$sleep_time" "$cmd"; then
		echo "Container $container_id is not running after ${wait_time}s"
		return 1
	fi
}

# Create a container and wait it be running.
#
# Parameters:
#	$1 - the pod name.
#	$2 - the pod configuration file.
#	$3 - the container configuration file.
#
crictl_create_cc_container() {
	local pod_name="$1"
	local pod_config="$2"
	local container_config="$3"
	local container_id=""
	local pod_id=""

	if [[ ! -f "$pod_config" || ! -f "$container_config" ]]; then
		echo "Pod or container config file does not exist"
		return 1
	fi

	pod_id=$(sudo crictl pods --name ${pod_name} -q)
	container_id=$(sudo crictl create -with-pull "${pod_id}" \
		"${container_config}" "${pod_config}")

	if [ -z "$container_id" ]; then
		echo "Failed to create the container"
		return 1
	fi

	if ! sudo crictl start ${container_id}; then
		echo "Failed to start container $container_id"
		sudo crictl ps -a
		return 1
	fi

	if ! crictl_wait_cc_container_be_running "$container_id"; then
		sudo crictl ps -a
		return 1
	fi
}

# Toggle between true and false the service_offload configuration of
# the Kata agent.
#
# Parameters:
#	$1: "on" to activate the service, or "off" to turn it off.
#
# Environment variables:
#	RUNTIME_CONFIG_PATH - path to kata's configuration.toml. If it is not
#			      export then it will figure out the path via
#			      `kata-runtime env` and export its value.
#
switch_image_service_offload() {
	# Load the RUNTIME_CONFIG_PATH variable.
	load_RUNTIME_CONFIG_PATH

	case "$1" in
		"on")
			sudo sed -i -e 's/^# *\(service_offload\).*=.*$/\1 = true/g' \
				"$RUNTIME_CONFIG_PATH"
			;;
		"off")
			sudo sed -i -e 's/^\(service_offload\).*=.*$/#\1 = false/g' \
				"$RUNTIME_CONFIG_PATH"

			;;
		*)
			echo "Unknown option $1"
			return 1
			;;
	esac
}

# Add parameters to the 'kernel_params' property on kata's configuration.toml
#
# Parameters:
#	$1..$N - list of parameters
#
# Environment variables:
#	RUNTIME_CONFIG_PATH - path to kata's configuration.toml. If it is not
#			      export then it will figure out the path via
#			      `kata-runtime env` and export its value.
#
add_kernel_params() {
	local params="$@"
	load_RUNTIME_CONFIG_PATH

	sudo sed -i -e 's#^\(kernel_params\) = "\(.*\)"#\1 = "\2 '"$params"'"#g' \
		"$RUNTIME_CONFIG_PATH"
}

# Get the 'kernel_params' property on kata's configuration.toml
#
# Environment variables:
#	RUNTIME_CONFIG_PATH - path to kata's configuration.toml. If it is not
#			      export then it will figure out the path via
#			      `kata-runtime env` and export its value.
#
get_kernel_params() {
	load_RUNTIME_CONFIG_PATH

        local kernel_params=$(sed -n -e 's#^kernel_params = "\(.*\)"#\1#gp' \
                "$RUNTIME_CONFIG_PATH")
	echo "$kernel_params"
}

# Clear the 'kernel_params' property on kata's configuration.toml
#
# Environment variables:
#	RUNTIME_CONFIG_PATH - path to kata's configuration.toml. If it is not
#			      export then it will figure out the path via
#			      `kata-runtime env` and export its value.
#
clear_kernel_params() {
	load_RUNTIME_CONFIG_PATH

	sudo sed -i -e 's#^\(kernel_params\) = "\(.*\)"#\1 = ""#g' \
		"$RUNTIME_CONFIG_PATH"
}

# Enable the agent console so that one can open a shell with the guest VM.
#
# Environment variables:
#	RUNTIME_CONFIG_PATH - path to kata's configuration.toml. If it is not
#			      export then it will figure out the path via
#			      `kata-runtime env` and export its value.
#
enable_agent_console() {
	load_RUNTIME_CONFIG_PATH

	sudo sed -i -e 's/^# *\(debug_console_enabled\).*=.*$/\1 = true/g' \
		"$RUNTIME_CONFIG_PATH"
}

enable_full_debug() {
	# Load the RUNTIME_CONFIG_PATH variable.
	load_RUNTIME_CONFIG_PATH

	# Toggle all the debug flags on in kata's configuration.toml to enable full logging.
	sudo sed -i -e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' "$RUNTIME_CONFIG_PATH"

	# Also pass the initcall debug flags via Kernel parameters.
	add_kernel_params "agent.log=debug" "initcall_debug"
}

disable_full_debug() {
	# Load the RUNTIME_CONFIG_PATH variable.
	load_RUNTIME_CONFIG_PATH

	# Toggle all the debug flags off in kata's configuration.toml to enable full logging.
	sudo sed -i -e 's/^# *\(enable_debug\).*=.*$/\1 = false/g' "$RUNTIME_CONFIG_PATH"
}

# Configure containerd for confidential containers. Among other things, it ensures
# the CRI handler is configured to deal with confidential container.
#
# Parameters:
#	$1 - (Optional) file path to where save the current containerd's config.toml
#
configure_cc_containerd() {
	local saved_containerd_conf_file="${1:-}"
	local containerd_conf_file="/etc/containerd/config.toml"

	# Even if we are not saving the original file it is a good idea to
	# restart containerd because it might be in an inconsistent state here.
	sudo systemctl stop containerd
	sleep 5
	[ -n "$saved_containerd_conf_file" ] && \
		cp -f "$containerd_conf_file" "$saved_containerd_conf_file"
	sudo systemctl start containerd
	waitForProcess 30 5 "sudo crictl info >/dev/null"

	# Ensure the cc CRI handler is set.
	local cri_handler=$(sudo crictl info | \
		jq '.config.containerd.runtimes.kata.cri_handler')
	if [[ ! "$cri_handler" =~ cc ]]; then
		sudo sed -z -i 's/\([[:blank:]]*\)\(runtime_type = "io.containerd.kata.v2"\)/\1\2\n\1cri_handler = "cc"/' \
			"$containerd_conf_file"
	fi

	if [ "$(sudo crictl info | jq -r '.config.cni.confDir')" = "null" ]; then
		echo "    [plugins.cri.cni]
		  # conf_dir is the directory in which the admin places a CNI conf.
		  conf_dir = \"/etc/cni/net.d\"" | \
			  sudo tee -a "$containerd_conf_file"
	fi

	sudo systemctl restart containerd
	if ! waitForProcess 30 5 "sudo crictl info >/dev/null"; then
		die "containerd seems not operational after reconfigured"
	fi
	sudo iptables -P FORWARD ACCEPT
}

#
# Auxiliar functions.
#

# Export the RUNTIME_CONFIG_PATH variable if it not set already.
#
load_RUNTIME_CONFIG_PATH() {
	if [ -z "$RUNTIME_CONFIG_PATH" ]; then
		extract_kata_env
	fi
}
