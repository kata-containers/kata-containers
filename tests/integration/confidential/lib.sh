#!/bin/bash
# Copyright (c) 2021, 2022 IBM Corporation
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This provides generic functions to use in the tests.
#
[ -z "${BATS_TEST_FILENAME:-}" ] && set -o errexit -o errtrace -o pipefail -o nounset

source "${BATS_TEST_DIRNAME}/../../../lib/common.bash"
#source "${BATS_TEST_DIRNAME}/../../../.ci/lib.sh"
FIXTURES_DIR="${BATS_TEST_DIRNAME}/fixtures"
SHARED_FIXTURES_DIR="${BATS_TEST_DIRNAME}/../../confidential/fixtures"

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
	load_runtime_config_path

	case "$1" in
		"on")
			sudo sed -i -e 's/^\(service_offload\).*=.*$/\1 = true/g' \
				"$RUNTIME_CONFIG_PATH"
			;;
		"off")
			sudo sed -i -e 's/^\(service_offload\).*=.*$/\1 = false/g' \
				"$RUNTIME_CONFIG_PATH"

			;;
		*)
			die "Unknown option '$1'"
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
	load_runtime_config_path

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
	load_runtime_config_path

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
	load_runtime_config_path

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
	load_runtime_config_path

	sudo sed -i -e 's/^# *\(debug_console_enabled\).*=.*$/\1 = true/g' \
		"$RUNTIME_CONFIG_PATH"
}

enable_full_debug() {
	# Load the RUNTIME_CONFIG_PATH variable.
	load_runtime_config_path

	# Toggle all the debug flags on in kata's configuration.toml to enable full logging.
	sudo sed -i -e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' "$RUNTIME_CONFIG_PATH"

	# Also pass the initcall debug flags via Kernel parameters.
	add_kernel_params "agent.log=debug" "initcall_debug"
}

disable_full_debug() {
	# Load the RUNTIME_CONFIG_PATH variable.
	load_runtime_config_path

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
		sudo sed -i 's/\([[:blank:]]*\)\(runtime_type = "io.containerd.kata.v2"\)/\1\2\n\1cri_handler = "cc"/' \
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
load_runtime_config_path() {
	if [ -z "$RUNTIME_CONFIG_PATH" ]; then
		extract_kata_env
	fi
}

setup_skopeo_signature_files_in_guest() {
	setup_common_signature_files_in_guest
	cp_to_guest_img "${rootfs_directory}" "${SHARED_FIXTURES_DIR}/registries.d"
}

# TODO #3970 - when skopeo fully removed refactor this and delete policy.json in fixture
setup_common_signature_files_in_guest() {
	rootfs_directory="etc/containers/"
	signatures_dir="${SHARED_FIXTURES_DIR}/quay_verification/signatures"

	if [ ! -d "${signatures_dir}" ]; then
		sudo mkdir "${signatures_dir}"
	fi

	sudo tar -zvxf "${SHARED_FIXTURES_DIR}/quay_verification/signatures.tar" -C "${signatures_dir}"

	cp_to_guest_img "${rootfs_directory}" "${SHARED_FIXTURES_DIR}/quay_verification"
}

setup_offline_fs_kbc_signature_files_in_guest() {
	setup_common_signature_files_in_guest
	setup_offline_fs_kbc_agent_config_in_guest
	cp_to_guest_img "etc" "${SHARED_FIXTURES_DIR}/offline-fs-kbc/aa-offline_fs_kbc-resources.json"
}

setup_offline_fs_kbc_agent_config_in_guest() {
	local rootfs_agent_config="/etc/agent-config.toml"

	#clone_katacontainers_repo
	#sudo -E AA_KBC_PARAMS="offline_fs_kbc::null" HTTPS_PROXY="${HTTPS_PROXY:-${https_proxy:-}}" envsubst < ${katacontainers_repo_dir}/docs/how-to/data/confidential-agent-config.toml.in | sudo tee ${rootfs_agent_config}
	sudo -E AA_KBC_PARAMS="offline_fs_kbc::null" HTTPS_PROXY="${HTTPS_PROXY:-${https_proxy:-}}" envsubst < "${SHARED_FIXTURES_DIR}/confidential-agent-config.toml.in" | sudo tee ${rootfs_agent_config}

	# TODO #5173 - remove this once the kernel_params aren't ignored by the agent config
	# Enable debug log_level and debug_console access based on env vars
	[ -n "${DEBUG:-}" ] || echo "log_level = \"debug\"" | sudo tee -a "${rootfs_agent_config}"
	[ "${DEBUG_CONSOLE:-}" = "true" ] || echo -e "debug_console = true\ndebug_console_vport = 1026" | sudo tee -a "${rootfs_agent_config}"

	# Uncomment the 'ExecProcessRequest' endpoint as our test currently uses exec to check the container
	sudo sed -i 's/#\("ExecProcessRequest"\)/\1/g' "${rootfs_agent_config}"

	cp_to_guest_img "/tests/fixtures" "${rootfs_agent_config}"
	add_kernel_params "agent.config_file=/tests/fixtures/$(basename ${rootfs_agent_config})"
}
