#!/bin/bash
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/asserts.sh"

# Common setup for tests.
#
# Global variables exported:
#	$test_start_time     - test start time.
#	$pod_config          - path to default pod configuration file.
#	$sandbox_name        - the sandbox name set on default pod configuration.
#	$saved_kernel_params - saved the original list of kernel parameters.
#
setup_common() {
	export test_start_time="$(date +"%Y-%m-%d %H:%M:%S")"
	export sandbox_name="kata-cc-busybox-sandbox"
	export pod_config="${FIXTURES_DIR}/pod-config.yaml"

	echo "Delete any existing ${sandbox_name} pod"
	crictl_delete_cc_pod_if_exists "$sandbox_name"

	echo "Prepare containerd for Confidential Container"
	SAVED_CONTAINERD_CONF_FILE="/etc/containerd/config.toml.$$"
	configure_cc_containerd "$SAVED_CONTAINERD_CONF_FILE"

	# Note: ensure that intructions changing the kernel parameters are
	# executed *after* saving the original list.
	saved_kernel_params=$(get_kernel_params)
	export saved_kernel_params

	echo "Enable image service offload"
	switch_image_service_offload on

	# On CI mode we only want to enable the agent debug for the case of
	# the test failure to obtain logs.
	if [ "${CI:-}" == "true" ]; then
		echo "Enable full debug"
		enable_full_debug
	elif [ "${DEBUG:-}" == "true" ]; then
		echo "Enable full debug"
		enable_full_debug
		echo "Enable agent console"
		enable_agent_console
	fi

	# In case the tests run behind a firewall where images needed to be
	# fetched through a proxy.
	local https_proxy="${HTTPS_PROXY:-${https_proxy:-}}"
	if [ -n "$https_proxy" ]; then
		echo "Enable agent https proxy"
		add_kernel_params "agent.https_proxy=$https_proxy"

		local local_dns=$(grep nameserver /etc/resolv.conf \
			/run/systemd/resolve/resolv.conf  2>/dev/null \
			|grep -v "127.0.0.53" | cut -d " " -f 2 | head -n 1)
		local new_file="${BATS_FILE_TMPDIR}/$(basename ${pod_config})"
		echo "New pod configuration with local dns: $new_file"
		cp -f "${pod_config}" "${new_file}"
		pod_config="$new_file"
		sed -i -e 's/8.8.8.8/'${local_dns}'/' "${pod_config}"
		cat "$pod_config"
	fi
}

# Common teardown for tests. Use alongside setup_common().
#
teardown_common() {
	# Allow to not destroy the environment if you are developing/debugging
	# tests.
	if [[ "${CI:-false}" == "false" && "${DEBUG:-}" == true ]]; then
		echo "Leaving changes and created resources untoughted"
		return
	fi

	crictl_delete_cc_pod_if_exists "$sandbox_name" || true

	# Restore the kernel parameters set before the test.
	clear_kernel_params
	[ -n "$saved_kernel_params" ] && \
		add_kernel_params "$saved_kernel_params"

	switch_image_service_offload off
	disable_full_debug

	# Restore containerd to pre-test state.
	if [ -f "$SAVED_CONTAINERD_CONF_FILE" ]; then
		systemctl stop containerd || true
		sleep 5
		mv -f "$SAVED_CONTAINERD_CONF_FILE" "/etc/containerd/config.toml"
		systemctl start containerd || true
	fi
}

# Create the test pod. Use alongside setup_common().
#
# Note: the global $pod_config should be set already.
#
create_test_pod() {
	echo "Create the test sandbox"
	crictl_create_cc_pod "$pod_config"
}
