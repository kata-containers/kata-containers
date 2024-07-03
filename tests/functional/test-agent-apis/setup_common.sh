#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

# 1. Setup: start kata-agent as an individual process listening on the default port
# 2. Use a bats script to run all tests
# 3. Collect logs if possible (look into this to validate success/failures)
# 4. handle errors/exit in cleanup to remove all mounts, etc.
# IMP: Look at how to assert tests.
# The agent process runs locally as a standalone process for now.

agent_binary="/usr/bin/kata-agent"
agent_ctl_path="/opt/kata/bin/kata-agent-ctl"

# Name of socket file used by a local agent.
agent_socket_file="/tmp/kata-agent.socket"

# Kata Agent socket URI.
local_agent_server_addr="unix://@${agent_socket_file}"

# Log file that contains agent ctl output
ctl_log_file="${PWD}/agent-ctl.log"
# Log file that contains agent output.
agent_log_file="${PWD}/kata-agent.log"

agent_log_level="debug"
keep_logs=false

cleanup()
{
	info "cleaning resources..."

	local failure_ret="$?"

	stop_agent

	local sandbox_dir="/run/sandbox-ns/"
	sudo umount -f "${sandbox_dir}/uts" "${sandbox_dir}/ipc" &>/dev/null || true
	sudo rm -rf "${sandbox_dir}" &>/dev/null || true

	if [ "$failure_ret" -eq 0 ] && [ "$keep_logs" = 'true' ]
	then
		info "SUCCESS: Test passed, but leaving logs:"
		info ""
		info "agent log file       : ${agent_log_file}"
		info "agent-ctl log file   : ${ctl_log_file}"
		return 0
	fi

	if [ $failure_ret -ne 0 ]; then
		warn "ERROR: Test failed"
		warn ""
		warn "Not cleaning up to help debug failure:"
		warn ""

		info "agent-ctl log file   : ${ctl_log_file}"
		info "agent log file       : ${agent_log_file}"
		return 0
	fi

	sudo rm -f \
		"$agent_log_file" \
		"$ctl_log_file"
}

run_agent_ctl()
{
	local cmds="${1:-}"

	[ -n "$cmds" ] || die "need commands for agent control tool"

	local redirect="&>\"${ctl_log_file}\""

	local server_address="--server-address ${local_agent_server_addr}"

	eval \
		sudo \
		RUST_BACKTRACE=full \
		"${agent_ctl_path}" \
		-l debug \
		connect \
		--no-auto-values \
		${server_address} \
		${cmds} \
		${redirect}
}

get_agent_pid()
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

check_agent_alive()
{
	local cmds=()

	cmds+=("-c Check")

	run_agent_ctl \
		"${cmds[@]}"
}

wait_for_agent_to_start()
{
	local cmd="check_agent_alive"

	local wait_time_secs=20
	local sleep_time_secs=1

	info "Waiting for agent process to start.."

	waitForProcess \
		"$wait_time_secs" \
		"$sleep_time_secs" \
		"$cmd"

	info "Kata agent process running."
}

stop_agent() {
	info "Stopping agent"
	local cmds=()
	cmds+=("-c DestroySandbox")
	run_agent_ctl \
		"${cmds[@]}"
}

start_agent()
{
	local log_file="${1:-}"
	[ -z "$log_file" ] && die "need agent log file"

	local running
	running=$(get_agent_pid || true)

	[ -n "$running" ] && die "agent already running: '$running'"

	eval \
		sudo \
			RUST_BACKTRACE=full \
			KATA_AGENT_LOG_LEVEL=${agent_log_level} \
			KATA_AGENT_SERVER_ADDR=${local_agent_server_addr} \
			${agent_binary} \
			&> ${log_file} \
			&

    wait_for_agent_to_start
}

setup_agent() {
	info "Starting a single kata agent process."

	start_agent $agent_log_file

	info "Setup done."
}
