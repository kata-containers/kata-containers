#!/bin/bash
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#---------------------------------------------------------------------
# Description: Test the Kata Containers 2.x rust agent shutdown behaviour.
#
#   Normally, the kata-agent process running inside the VM is not shut down;
#   once the workload ends and the agent has returned the workload return
#   value back to the runtime, the runtime simply kills the VM. This is safe
#   since nothing the user cares about is running any more.
#
#   However, for agent tracing, a graceful agent shutdown is necessary to ensure
#   all trace spans are generated. When *static* agent tracing is enabled, the
#   runtime relies entirely on the agent to perform a graceful shutdown _and_
#   shut down the VM.
#
#   This script tests the kata-agent in two ways:
#
#   - "manually" / "standalone" where the agent binary is run directly.
#   - Inside a Kata VM, started by a shimv2-capable container manager
#     (containerd).
#
#   In both cases, the agent is shut down using the agent-ctl tool
#   to request the agent shut down gracefully.
#
#   Various configuration options are also tested. One of these enables
#   the agents built-in (VSOCK) debug console. This test not only enables
#   the option but also connects to the created console.
#
#   Since this script needs to start various programs with a terminal,
#   it uses tmux(1) consistently to simplify the handling logic.
#---------------------------------------------------------------------

readonly script_name=${0##*/}

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../common.bash"
source "/etc/os-release" || source "/usr/lib/os-release"

CTR_RUNTIME=${CTR_RUNTIME:-"io.containerd.kata.v2"}

# Kata always uses this value
EXPECTED_VSOCK_PORT="1024"

DOCKER_IMAGE=${DOCKER_IMAGE:-"busybox"}
CTR_IMAGE=${CTR_IMAGE:-"quay.io/prometheus/busybox:latest"}

# Number of times the test should be run
KATA_AGENT_SHUTDOWN_TEST_COUNT=${KATA_AGENT_SHUTDOWN_TEST_COUNT:-1}

# Default VSOCK port used by the agent
KATA_AGENT_VSOCK_CONSOLE_PORT=${KATA_AGENT_VSOCK_CONSOLE_PORT:-1026}

# The shutdown test type that represents a "default" / vanilla Kata
# installation (where no debug options are enabled).
VANILLA_TEST_TYPE='default'

# Name of tmux(1) sessions to create to run Kata VM and local agent in
KATA_TMUX_VM_SESSION="kata-shutdown-test-vm-session"
KATA_TMUX_LOCAL_SESSION="kata-shutdown-test-local-agent-session"

# Name of tmux(1) session to create to run a debug console in
KATA_TMUX_CONSOLE_SESSION="kata-shutdown-test-console-session"

# tmux(1) session to run the trace forwarder in
KATA_TMUX_FORWARDER_SESSION="kata-shutdown-test-trace-forwarder-session"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

# List of test types used by configure_kata().
#
# Each element contains four colon delimited fields:
#
# 1: Name.
# 2: Whether debug should be enabled for the agent+runtime.
# 3: Whether hypervisor debug should be enabled.
#    (handled separately due to a previous bug which blocked agent shutdown).
# 4: Whether a VSOCK debug console should be configured and used.
#
# Notes:
#
# - Tests are run in the order found in this array.
# - An array is used (rather than a hash) to ensure the standard/vanilla
#   configuration is run *last*. The reason for this being that debug is
#   needed to diagnose shutdown errors, so there is no point in runnning
#   the default scenario first, in case it fails (and it thus "undebuggable").
shutdown_test_types=(
	'with-debug:true:false:false'
	'with-debug-console:false:false:true'
	'with-hypervisor-debug:true:true:false'
	'with-everything:true:true:true'
	"${VANILLA_TEST_TYPE}:false:false:false"
)

# Number of fields each entry in the 'shutdown_test_types' array should have.
shutdown_test_type_fields=4

# Pseudo test type name that represents all test types defined
# in the 'shutdown_test_types' array.
ALL_TEST_TYPES='all'

DEFAULT_SHUTDOWN_TEST_TYPE="${ALL_TEST_TYPES}"

# List of ways of running the agent:
#
# Each element contains two colon delimited fields:
#
# 1: Name used for a particular way of running the agent.
# 2: Description.
agent_test_types=(
	'local:Run agent using agent-ctl tool'
	'vm:Run agent inside a Kata Container'
)

# Default value from the 'agent_test_types' array.
DEFAULT_AGENT_TEST_TYPE='vm'

# Set by every call to run_single_agent()
test_start_time=
test_end_time=

#-------------------------------------------------------------------------------
# Settings

# values used to wait for local and VM processes to start and end.
wait_time_secs=${WAIT_TIME_SECS:-20}
sleep_time_secs=${SLEEP_TIME_SECS:-1}

# Time to allow for the agent and VM to shutdown
shutdown_time_secs=${SHUTDOWN_TIME_SECS:-120}

# Name for the container that will be created
container_id="${CONTAINER_ID:-kata-agent-shutdown-test}"

# If 'true', don't run any commands, just show what would be run.
dry_run="${DRY_RUN:-false}"

# If 'true', don't remove logs on a successful run.
keep_logs="${KEEP_LOGS:-false}"

# Name of socket file used by a local agent.
agent_socket_file="kata-agent.socket"

# Kata Agent socket URI.
#
# Notes:
#
# - The file is an abstract socket
#   (meaning it is not visible in the filesystem).
#
# - The agent and the agent-ctl tool use slightly different
#   address formats for abstract sockets.
local_agent_server_addr="unix://${agent_socket_file}"
local_agent_ctl_server_addr="unix://@${agent_socket_file}"

# Address that is dynamically configured when using CLH before
# starting trace forwarder or container
clh_socket_path=
clh_socket_prefix="/run/vc/vm/"

ctl_log_file="${PWD}/agent-ctl.log"

# Log file that must contain agent output.
agent_log_file="${PWD}/kata-agent.log"

# Set in setup() based on KATA_HYPERVISOR
# Supported hypervisors are qemu and clh
configured_hypervisor=
# String that would appear in config file (qemu or clh)
configured_hypervisor_cfg=

# Full path to directory containing an OCI bundle based on "$DOCKER_IMAGE",
# which is required by the agent control tool.
bundle_dir=${BUNDLE_DIR:-""}

#---------------------------------------
# Default values

default_arch=$(uname -m)
arch="${arch:-${default_arch}}"

#-------------------------------------------------------------------------------

agent_binary="/usr/bin/kata-agent"

# Maximum debug level
default_agent_log_level="trace"

agent_log_level=${agent_log_level:-${default_agent_log_level}}

# Full path to the main configuration file (set by setup()).
kata_cfg_file=

# Set in setup() based on KATA_HYPERVISOR
hypervisor_binary=


#-------------------------------------------------------------------------------

[ -n "${DEBUG:-}" ] && set -o xtrace

usage()
{
	cat <<EOF
Usage: $script_name [options]

Summary: Run Kata Agent shutdown tests.

Description: Run a set of tests to ensure the Kata Containers agent process
running inside the virtual machine can shut down cleanly. This is required for
static tracing. A number of variations of the test are run to exercise as many
code paths as possible, specifically different code paths for when particular
debug options are enabled.

Options:

 -a <agent-test-type>    : Agent test type to use
                           (default: '$DEFAULT_AGENT_TEST_TYPE').
 -c <count>              : Run specified number of iterations
                           (default: $KATA_AGENT_SHUTDOWN_TEST_COUNT).
 -d                      : Enable debug (shell trace) output.
 -h                      : Show this help statement.
 -k                      : Keep logs on successful run
			   (default: logs will be deleted on success).
 -l                      : List all available agent and shutdown test types.
 -n                      : Dry-run mode - show the commands that would be run.
 -t <shutdown-test-type> : Only run the specified shutdown test type
                           (default: '$DEFAULT_SHUTDOWN_TEST_TYPE').

Notes:

- These tests should be run *before* the Kata Agent tracing tests, since if
  the agent cannot be shut down, static tracing will not work reliably.

- By default all shutdown test types are run, but only the default agent test
  type is run.

EOF
}

warn()
{
	echo >&2 "WARNING: $*"
}

# Run the specified command, or if dry-run mode is enabled,
# just show the command that would be run.
run_cmd()
{
    local cmdline="$@"

    if [ "$dry_run" = 'true' ]
    then
        info "dry-run: Would run: '$cmdline'"
    else
        eval $cmdline
    fi
}

# Show a subset of processes (for debugging)
show_procs()
{
	info "Processes"

	local hypervisor
	hypervisor="qemu"
	[ ${configured_hypervisor} = "clh" ] && hypervisor="cloud-hypervisor"

	local patterns=()

	patterns+=("kata-agent-ctl")
	patterns+=("${hypervisor}")
	patterns+=("containerd")
	patterns+=("ctr")

	local pattern_list
	pattern_list=$(echo "${patterns[@]}"|tr ' ' '|')

	local regex
	regex="(${pattern_list})"

	ps -efww | grep -i -E "$regex" || true
}

kill_tmux_sessions()
{
	local session

	for session in \
		"$KATA_TMUX_CONSOLE_SESSION" \
		"$KATA_TMUX_FORWARDER_SESSION" \
		"$KATA_TMUX_LOCAL_SESSION" \
		"$KATA_TMUX_VM_SESSION"
		do
			tmux kill-session -t "$session" &>/dev/null || true
	done

	true
}

get_shutdown_test_type_entry()
{
	local shutdown_test_type="${1:-}"
	[ -z "$shutdown_test_type" ] && die "need shutdown test type name"

	local entry

	for entry in "${shutdown_test_types[@]}"
	do
		local count
		count=$(echo "$entry"|tr ':' '\n'|wc -l)
		[ "$count" -eq "$shutdown_test_type_fields" ] \
			|| die "expected $shutdown_test_type_fields fields, found $count: '$entry'"

		local name

		name=$(echo "$entry"|cut -d: -f1)

		[ "$name" = "$shutdown_test_type" ] \
				&& echo "$entry" \
				&& break
	done

	echo
}

list_shutdown_test_types()
{
	local entry
	local debug_value
	local hypervisor_debug_value
	local debug_console_value

	printf "# Shutdown test types:\n\n"

	printf "%-24s %-15s %-23s %s\n\n" \
		"Test type" \
		"Debug enabled" \
		"Hypervisor debug" \
		"Debug console used"

	for entry in "${shutdown_test_types[@]}"
	do
		local name
		local debug_value
		local hypervisor_debug_value
		local debug_console_value

		name=$(echo "$entry"|cut -d: -f1)
		debug_value=$(echo "$entry"|cut -d: -f2)
		hypervisor_debug_value=$(echo "$entry"|cut -d: -f3)
		debug_console_value=$(echo "$entry"|cut -d: -f4)

		printf "%-24s %-15s %-23s %s\n" \
			"$name" \
			"$debug_value" \
			"$hypervisor_debug_value" \
			"$debug_console_value"
	done

	echo
}

list_agent_test_types()
{
	local entry

	printf "# Agent test types:\n\n"

	printf "%-12s %s\n\n" \
		"Agent type" \
		"Description"

	for entry in "${agent_test_types[@]}"
	do
		local name
		local descr

		name=$(echo "$entry"|cut -d: -f1)
		descr=$(echo "$entry"|cut -d: -f2-)

		local msg=""

		[ "$name" = "$DEFAULT_AGENT_TEST_TYPE" ] && msg=" (default)"

		printf "%-12s %s%s.\n" \
				"$name" \
				"$descr" \
				"$msg"
	done

	echo
}

list_test_types()
{
	list_agent_test_types
	list_shutdown_test_types
}

# Set Kata options according to test type.
configure_kata()
{
	local shutdown_test_type="${1:-}"
	[ -z "$shutdown_test_type" ] && die "need shutdown test type"

	local entry
	local debug_value
	local hypervisor_debug_value
	local debug_console_value

	local entry
	entry=$(get_shutdown_test_type_entry "$shutdown_test_type" || true)
	[ -z "$entry" ] && die "invalid test type: '$shutdown_test_type'"

	debug_value=$(echo "$entry"|cut -d: -f2)
	hypervisor_debug_value=$(echo "$entry"|cut -d: -f3)
	debug_console_value=$(echo "$entry"|cut -d: -f4)

	[ -z "$debug_value" ] && \
		die "need debug value for $shutdown_test_type"

	[ -z "$hypervisor_debug_value" ] && \
		die "need hypervisor debug value for $shutdown_test_type"

	[ -z "$debug_console_value" ] && \
		die "need debug console value for $shutdown_test_type"

	toggle_debug "$debug_value" "$hypervisor_debug_value"
	toggle_vsock_debug_console "$debug_console_value"

	# Enable agent tracing
	#
	# Even though this program only tests agent shutdown, static tracing
	# must be configured. This is because normally (with tracing
	# disabled), the runtime kills the VM after the workload has exited.
	# However, if static tracing is enabled, the runtime will not kill the
	# VM - the responsibility for shutting down the VM is given to the
	# agent process running inside the VM.

	if [ "$shutdown_test_type" = "$VANILLA_TEST_TYPE" ]
	then
		# We don't need to worry about the 'trace_mode' here since agent tracing
		# is *only* enabled if the 'enable_tracing' variable is set.
		run_cmd sudo crudini --set "${kata_cfg_file}" 'agent.kata' 'enable_tracing' 'false'
	else
		run_cmd sudo crudini --set "${kata_cfg_file}" 'agent.kata' 'enable_tracing' 'true'
	fi
}

unconfigure_kata()
{
	info "Resetting configuration to defaults"

	configure_kata "$VANILLA_TEST_TYPE"
}

# Enable/disable the agent's built-in VSOCK debug console
toggle_vsock_debug_console()
{
	run_cmd sudo crudini --set "${kata_cfg_file}" \
		'agent.kata' 'debug_console_enabled' "$1"
}

# Enable/disable debug options.
#
# Note: Don't use 'kata-manager.sh "enable-debug"' since this
# enables all debug (including the problematic hypervisor
# debug - see below).
toggle_debug()
{
	local value="${1:-}"
	local hypervisor_debug="${2:-}"

	[ -z "$value" ] && die "need value"
	[ -z "$hypervisor_debug" ] && die "need hypervisor debug value"

	# list of confguration.toml sections that have debug options we care about
	local debug_sections=()

	debug_sections+=('agent.kata')
	debug_sections+=('runtime')

	local section

	for section in "${debug_sections[@]}"
	do
		run_cmd sudo crudini --set "$kata_cfg_file" "$section" \
			'enable_debug' "$value"
	done

	# XXX: Enabling hypervisor debug for QEMU will make a systemd debug
	# console service inoperable (*), but we need to test it anyhow.
	#
	# (*) - If enabled, it stops "kata-debug.service" from attaching to
	# the console and the socat call made on the client hangs until
	# the VM is shut down!
	local section

	section=$(printf "hypervisor.%s" "$configured_hypervisor_cfg")

	run_cmd sudo crudini --set "$kata_cfg_file" "$section" \
		'enable_debug' "$hypervisor_debug_value"
}

# Provide a "semi-valid" vsock address for when dry-run mode is active.
# The URI includes a message telling the user to change it and replace
# with the real VSOCK CID value.
get_dry_run_agent_vsock_address()
{
	echo "vsock://FIXME-CHANGE-TO-VSOCK-CID:${EXPECTED_VSOCK_PORT}"
}

# Start a debug console shell using the agent's built-in debug console
# feature.
#
# Note: You should be able to use "kata-runtime exec $cid", but that isn't
# working currently.
connect_to_vsock_debug_console()
{
	local agent_addr

	if [ "$dry_run" = 'true' ]
	then
		agent_addr=$(get_dry_run_agent_vsock_address)
	else
		agent_addr=$(get_agent_vsock_address || true)
		[ -z "$agent_addr" ] && die "cannot determine agent VSOCK address"
	fi

	local socat_connect=
	if [ $configured_hypervisor = "qemu" ]; then
		socat_connect=$(echo "$agent_addr"|sed 's!^vsock://!vsock-connect:!')
	elif [ $configured_hypervisor = "clh" ]; then
		socat_connect="unix-connect:${clh_socket_path}"
	else
		die "Cannot configure address for socat, unknown hypervisor: '$configured_hypervisor'"
	fi

	run_cmd \
		"tmux new-session \
		-d \
		-s \"$KATA_TMUX_CONSOLE_SESSION\" \
		\"socat \
			'${socat_connect}' \
			stdout\""

}

cleanup()
{
	# Save the result of the last call made before
	# this handler was called.
	#
	# XXX: This *MUST* be the first command in this function!
	local failure_ret="$?"

	[ "$dry_run" = 'true' ] && return 0

	if [ "$failure_ret" -eq 0 ] && [ "$keep_logs" = 'true' ]
	then
		info "SUCCESS: Test passed, but leaving logs:"
		info ""
		info "agent log file       : ${agent_log_file}"
		info "agent-ctl log file   : ${ctl_log_file}"
		info "OCI bundle directory : ${bundle_dir}"

		return 0
	fi

	local arg="${1:-}"

	if [ $failure_ret -ne 0 ] && [ "$arg" != 'initial' ]; then
		warn "ERROR: Test failed"
		warn ""
		warn "Not cleaning up to help debug failure:"
		warn ""

		info "agent-ctl log file   : ${ctl_log_file}"
		info "agent log file       : ${agent_log_file}"

		info "OCI bundle directory : ${bundle_dir}"

		return 0
	fi

	kill_tmux_sessions

	unconfigure_kata

	 [ "$arg" != 'initial' ] && [ -d "$bundle_dir" ] && rm -rf "$bundle_dir"

	sudo rm -f \
		"$agent_log_file" \
		"$ctl_log_file"

	clean_env_ctr &>/dev/null || true

	local sandbox_dir="/run/sandbox-ns/"

	# XXX: Without doing this, the agent will hang attempting to create the
	# XXX: namespaces (in function "setup_shared_namespaces()")
	sudo umount -f "${sandbox_dir}/uts" "${sandbox_dir}/ipc" &>/dev/null || true
	sudo rm -rf "${sandbox_dir}" &>/dev/null || true

	# Check that clh socket was deleted
	if [ $configured_hypervisor = "clh" ] && [ ! -z $clh_socket_path ]; then
		[ -f $clh_socket_path ] && die "CLH socket path $clh_socket_path was not properly cleaned up"
	fi

	sudo systemctl restart containerd
}

setup_containerd()
{
	local file="/etc/containerd/config.toml"

	[ -e "$file" ] || die "missing containerd config file: '$file'"

	# Although the containerd config file is in TOML format, crudini(1)
	# won't parse it due to the indentation it uses.
	local containerd_debug_enabled

	containerd_debug_enabled=$(sed \
		-e '/./{H;$!d;}' \
		-e 'x;/\[debug\]/!d;' \
		"$file" |\
		grep "level *= *\"debug\"" || true)

	if [ -z "$containerd_debug_enabled" ]
	then
		cat <<-EOF | sudo tee -a "$file"
		[debug]
		    # Allow Kata Containers debug messages to be propageted
		    # into the hosts journal.
		    # (use "journalctl -t kata" to view).
		    level = "debug"
		EOF

		sudo systemctl restart containerd
	fi

	sudo ctr image pull "$CTR_IMAGE"

	true
}

create_oci_rootfs()
{
	local dir="${1:-}"

	[ -z "$dir" ] && die "Need OCI rootfs dir"

	sudo docker export $(sudo docker create "$DOCKER_IMAGE") |\
		tar -C "${dir}" -xvf - >/dev/null
}

setup_oci_bundle()
{
	bundle_dir="$(mktemp -d)"
	export bundle_dir

	info "Creating OCI bundle in directory: '$bundle_dir'"

	local config="${bundle_dir}/config.json"
	local rootfs_dir="${bundle_dir}/rootfs/"

	mkdir -p "$rootfs_dir"

	create_oci_rootfs "$rootfs_dir"

	pushd "$bundle_dir" &>/dev/null
	runc spec
	popd &>/dev/null

	[ -e "$config" ] || die "no OCI config file at ${config}"
}

setup()
{
	configured_hypervisor="${KATA_HYPERVISOR:-}"

	if [ "${KATA_HYPERVISOR:-}" = "qemu" ]; then
		hypervisor_binary="qemu-system-${arch}"
		configured_hypervisor_cfg="qemu"
	elif [ "${KATA_HYPERVISOR:-}" = "clh" ]; then
		hypervisor_binary="cloud-hypervisor"
		configured_hypervisor_cfg="clh"
	else
		local msg=""
		msg+="Exiting as hypervisor test dependency not met"
		msg+=" (expected 'qemu' or 'cloud-hypervisor', found '$KATA_HYPERVISOR')"
		die "$msg"
	fi
	info "Configured hypervisor is $configured_hypervisor"

	trap cleanup EXIT

	# Don't mess with an existing tmux session
	unset TMUX

	[ "$dry_run" = 'false' ] && \
		[ -z "$bundle_dir" ] && \
		setup_oci_bundle || true

	local cmds=()

	# For parsing TOML config files
	cmds+=('crudini')

	# For container manager (containerd)
	cmds+=('ctr')

	# for OCI bundle creation
	cmds+=('docker')
	cmds+=('runc')

	# For querying VSOCK sockets
	cmds+=('socat')

	# For launching processes
	cmds+=('tmux')

	local cmd

	for cmd in "${cmds[@]}"
	do
		local result
		result=$(command -v "$cmd" || true)
		[ -n "$result" ] || die "need $cmd"
	done

	kata_cfg_file=$(kata-runtime kata-env \
		--json |\
		jq '.Runtime | .Config | .Path' |\
		cut -d\" -f2 || true)

	[ -z "$kata_cfg_file" ] && die "Cannot determine config file"

	sudo mkdir -p $(dirname "$kata_cfg_file")

	#------------------------------
	# Check configured hypervisor

	local hypervisor_section

	hypervisor_section=$(printf "hypervisor.%s\n" "${configured_hypervisor_cfg}")

	local ret

	{ crudini --get "${kata_cfg_file}" "${hypervisor_section}" &>/dev/null; ret=$?; } || true

	[ "$ret" -eq 0 ] || \
		die "Configured hypervisor ${configured_hypervisor} does not match config file ${kata_cfg_file}"

	setup_containerd
}

start_local_agent()
{
	local log_file="${1:-}"
	[ -z "$log_file" ] && die "need agent log file"

	local running
	running=$(get_local_agent_pid || true)

	[ -n "$running" ] && die "agent already running: '$running'"

	# Note: it's imperative that we capture stderr to the log file
	# as the agent writes the shutdown message to this stream!
	run_cmd \
		"tmux new-session \
		-d \
		-s \"$KATA_TMUX_LOCAL_SESSION\" \
		\"sudo \
			RUST_BACKTRACE=full \
			KATA_AGENT_LOG_LEVEL=${agent_log_level} \
			KATA_AGENT_SERVER_ADDR=${local_agent_server_addr} \
			${agent_binary} \
			&> ${log_file}\""

	[ "$dry_run" = 'false' ] && wait_for_local_agent_to_start || true
}

# Wait for the agent to finish starting
wait_for_kata_vm_agent_to_start()
{
	local cid="${1:-}"
	[ -z "$log_file" ] && die "need container ID"

	# First, check the containerd status of the container
	local cmd="sudo ctr task list | grep \"${cid}\" | grep -q \"RUNNING\""

	info "Waiting for VM to start (cid: '$cid')"

	waitForProcess \
		"$wait_time_secs" \
		"$sleep_time_secs" \
		"$cmd"

	show_procs

	# Next, ensure there is a valid VSOCK address for the VM
	info "Waiting for agent VSOCK server"

	cmd="get_agent_vsock_address_simple >/dev/null"

	waitForProcess \
		"$wait_time_secs" \
		"$sleep_time_secs" \
		"$cmd"

	info "Kata VM running"
}

check_local_agent_alive()
{
	local cmds=()

	cmds+=("-c Check")

	run_agent_ctl \
		"${local_agent_ctl_server_addr}" \
		"${cmds[@]}"

	true
}

wait_for_local_agent_to_start()
{
	local cmd="check_local_agent_alive"

	info "Waiting for agent process to start"

	waitForProcess \
		"$wait_time_secs" \
		"$sleep_time_secs" \
		"$cmd"

	info "Kata agent process running"
}

# Create a Kata Container that blocks "forever"
start_agent_in_kata_vm()
{
	local log_file="${1:-}"
	[ -z "$log_file" ] && die "need agent log file"

	local snapshotter=""
	local ret

	# Allow containerd to run on a ZFS root filesystem
	{ zfs list &>/dev/null; ret=$?; } || true
	[ "$ret" = 0 ] && snapshotter='zfs'

	# Ensure the container blocks forever
	local cmd='tail -f /dev/null'

	run_cmd \
		"tmux new-session \
		-d \
		-s \"$KATA_TMUX_VM_SESSION\" \
		\"sudo ctr run \
			--snapshotter '$snapshotter' \
			--runtime '${CTR_RUNTIME}' \
			--rm \
			-t '${CTR_IMAGE}' \
			'$container_id' \
			$cmd\""

	[ "$dry_run" = 'false' ] && \
		wait_for_kata_vm_agent_to_start "$container_id" || true
}

start_agent()
{
	local agent_test_type="${1:-}"
	[ -z "$agent_test_type" ] && die "need agent test type"

	local log_file="${2:-}"
	[ -z "$log_file" ] && die "need agent log file"

	case "$agent_test_type" in
		'local') start_local_agent "$log_file" ;;
		'vm') start_agent_in_kata_vm "$log_file" ;;
		*) die "invalid agent test type: '$agent_test_type'" ;;
	esac

	true
}

run_agent_ctl()
{
	local server_addr="${1:-}"

	shift

	local cmds="${*:-}"

	[ -n "$server_addr" ] || die "need agent ttRPC server address"
	[ -n "$cmds" ] || die "need commands for agent control tool"

	local agent_ctl_path
	agent_ctl_path="/opt/kata/bin/kata-agent-ctl"

	local redirect="&>\"${ctl_log_file}\""

	if [ "$dry_run" = 'true' ]
	then
		redirect=""
		bundle_dir="FIXME-set-to-OCI-bundle-directory"
	fi

	local server_address=
	if [ $configured_hypervisor = "qemu" ]; then
		server_address="--server-address \"${server_addr}\""
	elif [ $configured_hypervisor = "clh" ]; then
		server_address="--server-address \"${server_addr}\" --hybrid-vsock"
	else
		die "Cannot configure server address, unknown hypervisor: '$configured_hypervisor'"
	fi

	run_cmd \
		sudo \
		RUST_BACKTRACE=full \
		"${agent_ctl_path}" \
		-l debug \
		connect \
		"${server_address}" \
		--bundle-dir "${bundle_dir}" \
		"${cmds}" \
		"${redirect}"
}

# This function "cheats" a little - it gets the agent
# to do some work *and then* stops it.
stop_local_agent()
{
	local cmds=()

	cmds+=("-c Check")
	cmds+=("-c GetGuestDetails")
	cmds+=("-c 'sleep 1s'")
	cmds+=("-c DestroySandbox")

	run_agent_ctl \
		"${local_agent_ctl_server_addr}" \
		"${cmds[@]}"
}

get_addresses()
{
	local addresses=

	if [ $configured_hypervisor = "qemu" ]; then
                addresses=$(ss -Hp --vsock |\
                        grep -v -E "\<socat\>" |\
                        awk '$2 ~ /^ESTAB$/ {print $6}' |\
                        grep ":${EXPECTED_VSOCK_PORT}$")
	elif [ $configured_hypervisor = "clh" ]; then
                # since we preconfigured the socket, we are checking to see if it is reported
                addresses=$(ss -Hp |\
                        grep "${clh_socket_path}" |\
                        awk '$2 ~ /^ESTAB$/ {print $5}')
	else
		die "Cannot retrieve address, unknown hypervisor: '$configured_hypervisor'"
	fi

	echo ${addresses}
}

# Doesn't fail. Instead it will return the empty string on error.
get_agent_vsock_address_simple()
{
	local addresses=$(get_addresses)

	[ -z "$addresses" ] && return 1

	local expected_count=1

	local count
	count=$(echo "$addresses"|wc -l || true)

	[ "$count" -eq "$expected_count" ] || return 1

	if [ $configured_hypervisor = "qemu" ]; then
		local cid
		local port

		cid=$(echo "$addresses"|cut -d: -f1)
		port=$(echo "$addresses"|cut -d: -f2)

		echo "vsock://${cid}:${port}"
	elif [ $configured_hypervisor = "clh" ]; then
		address=$(echo "$addresses" | awk 'NR==1{print $1}')
		echo "unix://${address}"
	else
		die "Cannot get agent vsock address, unknown hypervisor: '$configured_hypervisor'"
	fi

	return 0
}

get_agent_vsock_address()
{
	local addresses=$(get_addresses)

	[ -z "$addresses" ] && die "no VSOCK connections found"

	local expected_count=1

	local count
	count=$(echo "$addresses"|wc -l || true)

	if [ $configured_hypervisor = "qemu" ]; then
		# For QEMU we always expect 1 result. For Cloud Hypervisor, if a debug console is configured
		# and running, we will have more than 1 result, so only run this check for QEMU
		[ "$count" -eq "$expected_count" ] \
			|| die "expected $expected_count VSOCK entry, found $count: '$addresses'"

		local cid
		local port

		cid=$(echo "$addresses"|cut -d: -f1)
		port=$(echo "$addresses"|cut -d: -f2)

		echo "vsock://${cid}:${port}"
	elif [ $configured_hypervisor = "clh" ]; then
		address=$(echo "$addresses" | awk 'NR==1{print $1}')
		echo "unix://${address}"
	else
		die "Cannot get agent vsock address, unknown hypervisor: '$configured_hypervisor'"
	fi
}

stop_agent_in_kata_vm()
{
	local agent_addr

	if [ "$dry_run" = 'true' ]
	then
		agent_addr=$(get_dry_run_agent_vsock_address)
	else
		agent_addr=$(get_agent_vsock_address || true)
		[ -z "$agent_addr" ] && \
			die "cannot determine agent VSOCK address for $hypervisor_binary"
	fi

	# List of API commands to send to the agent.
	local cmds=()

	# Run a couple of query commands first to ensure
	# the agent is listening.
	cmds+=("-c Check")
	cmds+=("-c GetGuestDetails")

	# Creating a container implies creating a sandbox, so request
	# agent/VM/container shutdown by asking the agent
	# to destroy the sandbox.
	cmds+=("-c DestroySandbox")

	run_agent_ctl \
		"${agent_addr}" \
		"${cmds[@]}"

	true
}

stop_agent()
{
	info "Stopping agent"

	local agent_test_type="${1:-}"
	[ -z "$agent_test_type" ] && die "need agent test type"

	local log_file="${2:-}"
	[ -z "$log_file" ] && die "need agent-ctl log file"

	case "$agent_test_type" in
		'local') stop_local_agent ;;
		'vm') stop_agent_in_kata_vm ;;
		*) die "invalid agent test type: '$agent_test_type'" ;;
	esac

	true
}

get_local_agent_pid()
{
	local pids

	local name
	name=$(basename "$agent_binary")

	pids=$(pgrep "$name" || true)
	[ -z "$pids" ] && return 0

	local count
	count=$(echo "$pids"|wc -l)

	[ "$count" -gt 1 ] && \
		die "too many agent processes running ($count, '$pids')"

	echo $pids
}

# Function that writes all agent logs to '$agent_log_file'.
get_agent_log_file()
{
	local agent_test_type="${1:-}"
	[ -z "$agent_test_type" ] && die "need agent test type"

	local log_file="${2:-}"
	[ -z "$log_file" ] && die "need agent log file"

	info "Getting agent log details"

	case "$agent_test_type" in
		# NOP: File should have been created by start_local_agent()
		'local') true ;;

		# Extract journal entries for the duration of the test
		'vm')
			sudo journalctl \
				-q \
				-a \
				-o cat \
				-t 'kata' \
				--since="$test_start_time" \
				> "$log_file"
		;;

		*) die "invalid agent test type: '$agent_test_type'" ;;
	esac

	[ -e "$log_file" ] || die "no log file: '$log_file'"
	[ -s "$log_file" ] || die "empty log file: '$log_file'"

	true
}

# Function to run to ensure correct behaviour
validate_agent()
{
	local agent_test_type="${1:-}"
	local shutdown_test_type="${2:-}"
	local log_file="${3:-}"

	[ -z "$agent_test_type" ] && die "need agent test type"
	[ -z "$shutdown_test_type" ] && die "need shutdown test type"
	[ -z "$log_file" ] && die "need agent log file"

	info "validating"

	get_agent_log_file \
		"$agent_test_type" \
		"$log_file"

	# Regular expression that describes possible agent failures
	local regex="(slog::Fuse|Drain|Custom|serialization error|thread.*panicked|stack backtrace:)"

	grep -q -E "$regex" "$log_file" && cat $log_file && die "Found agent error in log file: '$log_file'"

	local entry
	entry=$(get_shutdown_test_type_entry "$shutdown_test_type" || true)
	[ -z "$entry" ] && die "invalid test type: '$shutdown_test_type'"

	local hypervisor_debug=$(echo "$entry"|cut -d: -f3)
	local vsock_console=$(echo "$entry"|cut -d: -f4)

	local agent_debug_logs_available='false'

	[ "$hypervisor_debug" = 'true' ] && \
		[ "$vsock_console" = 'false' ] && \
		agent_debug_logs_available='true'

	if [ "$agent_debug_logs_available" = 'true' ] || [ "$agent_test_type" = 'local' ]
	then
		# The message the agent writes to stderr just before it exits.
		local done_msg="\<shutdown complete\>"

		grep -q -E "$done_msg" "$log_file" || (cat $log_file && die "missing agent shutdown message")
	else
		# We can only check for the shutdown message if the agent debug
		# logs are available.
		info "Not checking for agent shutdown message as hypervisor debug disabled"
	fi
}

setup_agent()
{
	local shutdown_test_type="${1:-}"
	[ -z "$shutdown_test_type" ] && die "need shutdown test type"

	kill_tmux_sessions

	configure_kata "$shutdown_test_type"

	true
}

# Even though this test is not testing tracing, agent tracing needs to be
# enabled to stop the runtime from killing the VM. However, if tracing is
# enabled, the forwarder must be running. To remove the need for Jaeger to
# also be running, run the forwarder in "NOP" mode.
run_trace_forwarder()
{
	local forwarder_binary_path
	forwarder_binary_path="/opt/kata/bin/kata-trace-forwarder"

	local socket_path_tf=""

	# If using CLH, socket path must be passed to trace forwarder
	if [ $configured_hypervisor = "clh" ]; then
		socket_path_tf="--socket-path ${clh_socket_path}"
	fi

	run_cmd \
		"tmux new-session \
		-d \
		-s \"$KATA_TMUX_FORWARDER_SESSION\" \
		sudo \"$forwarder_binary_path --dump-only -l trace ${socket_path_tf}\""
}

check_agent_stopped()
{
	info "Checking agent stopped"

	local agent_test_type="${1:-}"
	[ -z "$agent_test_type" ] && die "need agent test type"

	local cmd=

	case "$agent_test_type" in
		'local') cmd=check_local_agent_stopped ;;
		'vm') cmd=check_vm_stopped ;;
		*) die "invalid agent test type: '$agent_test_type'" ;;
	esac

	waitForProcess \
		"$shutdown_time_secs" \
		"$sleep_time_secs" \
		"$cmd"

	true
}

check_local_agent_stopped()
{
	local ret=0

	local i=0
	local max=20

	agent_ended="false"

	local agent_pid
	agent_pid=$(get_local_agent_pid || true)

	# Agent has finished
	[ -z "$agent_pid" ] && return 0

	for _ in $(seq "$max")
	do
		{ sudo kill -0 "$agent_pid"; ret=$?; } || true

		[ "$ret" -ne 0 ] && agent_ended="true" && break

		sleep 0.2
	done

	[ "$agent_ended" = "false" ] && die "agent still running: pid $agent_pid" || true
}

get_vm_pid()
{
	pgrep "$hypervisor_binary"
}

check_vm_stopped()
{
	tmux list-sessions |\
		grep -q "^${KATA_TMUX_VM_SESSION}:" \
		&& return 1

	return 0
}

start_debug_console()
{
	local agent_test_type="${1:-}"
	local shutdown_test_type="${2:-}"

	[ -z "$agent_test_type" ] && die "need agent test type"
	[ -z "$shutdown_test_type" ] && die "need shutdown test type"

	info "Starting debug console"

	case "$agent_test_type" in
		'vm') connect_to_vsock_debug_console ;;
		# NOP for a local agent since we cannot connect to the agents
		# VSOCK console socket from *outside* the host!
		'local') true ;;
		*) die "invalid agent test type: '$agent_test_type'" ;;
	esac

	true
}

run_single_agent()
{
	local agent_test_type="${1:-}"
	local shutdown_test_type="${2:-}"

	[ -z "$agent_test_type" ] && die "need agent test type"
	[ -z "$shutdown_test_type" ] && die "need shutdown test type"

	local msg
	msg=$(printf \
		"Testing agent (agent test type: '%s', shutdown test type: '%s')" \
		"$agent_test_type" \
		"$shutdown_test_type")
	info "$msg"

	setup_agent "$shutdown_test_type"

	if [ $configured_hypervisor = "clh" ]; then
		# CLH uses hybrid VSOCK which uses a local UNIX socket that we need to specify
		socket_path_template=$clh_socket_prefix$(sudo kata-runtime env --json | jq '.Hypervisor.SocketPath')
		clh_socket_path=$(echo "$socket_path_template" | sed "s/{ID}/${container_id}/g" | tr -d '"')
		[ "$dry_run" = 'false' ] && sudo mkdir -p $(dirname "$clh_socket_path")
	fi

	run_trace_forwarder "$shutdown_test_type"

	sleep 5s

	test_start_time=$(date '+%F %T')

	start_agent \
		"$agent_test_type" \
		"$agent_log_file"

	info "Testing agent: shutdown test type: '$shutdown_test_type', agent test type: $agent_test_type"

	local entry
	entry=$(get_shutdown_test_type_entry "$shutdown_test_type" || true)
	local debug_console=$(echo "$entry"|cut -d: -f4)
	[ "$debug_console" = 'true' ] && \
		start_debug_console \
		"$agent_test_type" \
		"$shutdown_test_type"

	stop_agent \
		"$agent_test_type" \
		"$ctl_log_file"

	# We only need to show the set of commands once
	[ "$dry_run" = 'true' ] && exit 0

	test_end_time=$(date '+%F %T')

	check_agent_stopped "$agent_test_type"

	validate_agent \
		"$agent_test_type" \
		"$shutdown_test_type" \
		"$agent_log_file"
}

run_agent()
{
	local agent_test_type="${1:-}"
	local shutdown_test_type="${2:-}"

	[ -z "$agent_test_type" ] && die "need agent test type"
	[ -z "$shutdown_test_type" ] && die "need shutdown test type"

	case "$shutdown_test_type" in
		"$ALL_TEST_TYPES")
			local entry

			# Run all shutdown types
			for entry in "${shutdown_test_types[@]}"
			do
				local name
				name=$(echo "$entry"|cut -d: -f1)

				run_single_agent \
					"$agent_test_type" \
					"$name"

				# Clean up between iterations
				sudo rm -f \
					"$ctl_log_file" \
					"$agent_log_file"

				local addresses=$(get_addresses || true)

				[ -z "$addresses" ] || \
					die "found unexpected vsock addresses: '$addresses'"

			done
		;;

		*)
			run_single_agent \
				"$agent_test_type" \
				"$shutdown_test_type"
		;;
	esac

}

test_agent_shutdown()
{
	local count="${1:-}"
	local agent_test_type="${2:-}"
	local shutdown_test_type="${3:-}"

	[ -z "$count" ] && die "need count"
	[ -z "$agent_test_type" ] && die "need agent test type"
	[ -z "$shutdown_test_type" ] && die "need shutdown test type"

	# Start with a clean environment
	[ "$dry_run" = 'false' ] && cleanup initial || true

	local i

	for i in $(seq "$count")
	do
		[ "$dry_run" = 'false' ] && \
			info "testing agent: run $i of $count" || true
		run_agent \
			"$agent_test_type" \
			"$shutdown_test_type"
	done

	info "testing agent: completed $count runs"
}

handle_args()
{
	local opt

	local count="${KATA_AGENT_SHUTDOWN_TEST_COUNT}"
	local shutdown_test_type="$DEFAULT_SHUTDOWN_TEST_TYPE"
	local agent_test_type="$DEFAULT_AGENT_TEST_TYPE"

	while getopts "a:c:dhklnt:" opt "$@"
	do
		case "$opt" in
			a) agent_test_type="$OPTARG" ;;
			c) count="$OPTARG" ;;
			d) set -o xtrace ;;
			h) usage; exit 0 ;;
			k) keep_logs='true' ;;
			l) list_test_types; exit 0 ;;
			n) dry_run='true' ;;
			t) shutdown_test_type="$OPTARG" ;;
			*) die "invalid option: '$opt'" ;;
		esac
	done

	setup

	test_agent_shutdown \
		"$count" \
		"$agent_test_type" \
		"$shutdown_test_type"
}

main()
{
	handle_args "$@"
}

main "$@"
