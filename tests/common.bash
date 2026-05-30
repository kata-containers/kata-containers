#!/usr/bin/env bash
#
# Copyright (c) 2018-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# This file contains common functions that
# are being used by our metrics and integration tests

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "${this_script_dir}/../" && pwd)"
export repo_root_dir

# shellcheck source=/dev/null
source "${this_script_dir}/error.sh"

# Kata tests directory used for storing various test-related artifacts.
KATA_TESTS_BASEDIR="${KATA_TESTS_BASEDIR:-/var/log/kata-tests}"

# Directory that can be used for storing test logs.
KATA_TESTS_LOGDIR="${KATA_TESTS_LOGDIR:-${KATA_TESTS_BASEDIR}/logs}"

# Directory that can be used for storing test data.
KATA_TESTS_DATADIR="${KATA_TESTS_DATADIR:-${KATA_TESTS_BASEDIR}/data}"

# Directory that can be used for storing cache kata components
KATA_TESTS_CACHEDIR="${KATA_TESTS_CACHEDIR:-${KATA_TESTS_BASEDIR}/cache}"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

RUNTIME="${RUNTIME:-containerd-shim-kata-v2}"

export branch="${target_branch:-"$(git remote show origin | sed -n '/HEAD branch/s/.*: //p')"}"

# Export target_branch to avoid hitting the remote repository again when this script gets loaded again.
export target_branch="${branch}"

function die() {
	local msg="$*"

	if [[ -z "${KATA_TEST_VERBOSE:-}" ]]; then
		echo -e "[$(basename "$0"):${BASH_LINENO[0]}] ERROR: ${msg}" >&2
		exit 1
	fi

	echo >&2 "ERROR: ${msg}"

	# This function is called to indicate a fatal error occurred, so
	# the caller of this function is the site of the detected error.
	local error_location
	error_location=$(caller 0)

	local line
	local func
	local file

	line=$(echo "${error_location}"|awk '{print $1}')
	func=$(echo "${error_location}"|awk '{print $2}')
	file=$(echo "${error_location}"|awk '{print $3}')

	local path
	path=$(resolve_path "${file}")

	dump_details \
		"${line}" \
		"${func}" \
		"${path}"

	exit 1
}

function warn() {
	local msg="$*"
	echo -e "[$(basename "$0"):${BASH_LINENO[0]}] WARNING: ${msg}"
}

function info() {
	local msg="$*"
	echo -e "[$(basename "$0"):${BASH_LINENO[0]}] INFO: ${msg}"
}

function bats_unbuffered_info() {
	local msg="$*"
	# Ask bats to print this text immediately rather than buffering until the end of a test case.
	echo -e "[$(basename "$0"):${BASH_LINENO[0]}] UNBUFFERED: INFO: ${msg}" >&3
}

# Create Docker config for genpolicy so it can authenticate when pulling image
# manifests. Genpolicy uses docker_credential::get_credential(), which reads
# DOCKER_CONFIG/config.json.
#
# Parameters:
#	$1	- explicit registry host such as nvcr.io, or an image reference
#		  with an explicit registry host such as nvcr.io/nim/meta/llama:latest
#	$2	- registry username
#	$3	- registry password (empty password leaves auth unchanged)
#	$4	- Docker config directory (default: ${kubernetes_dir}/.docker-genpolicy)
function setup_genpolicy_registry_auth() {
	local registry_or_image="${1:-}"
	local username="${2:-}"
	local password="${3:-}"
	local auth_dir="${4:-${kubernetes_dir:-${PWD}}/.docker-genpolicy}"

	[[ -n "${password}" ]] || return 0
	[[ -n "${registry_or_image}" ]] || die "Registry host or image reference not provided"
	[[ -n "${username}" ]] || die "Registry username not provided"

	local registry="${registry_or_image#http://}"
	registry="${registry#https://}"
	registry="${registry%%/*}"
	[[ -n "${registry}" ]] || die "Could not determine registry from ${registry_or_image}"

	mkdir -p "${auth_dir}"

	local auth
	auth=$(printf "%s" "${username}:${password}" | base64 -w0)

	printf '{"auths":{"%s":{"auth":"%s"}}}\n' "${registry}" "${auth}" > "${auth_dir}/config.json"
	chmod 600 "${auth_dir}/config.json"

	export DOCKER_CONFIG="${auth_dir}"
	export REGISTRY_AUTH_FILE="${auth_dir}/config.json"
}

function handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo -e "[$(basename "$0"):${line_number}] ERROR: $(eval echo "${BASH_COMMAND}")"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

# A wrapper function for kubectl with retry logic
# runs the command up to 5 times with a 15-second interval by default
# to ensure successful execution
# Usage:
#   kubectl_retry [max_tries] [interval] kubectl_args...
#   kubectl_retry kubectl_args...  (uses defaults: 5 retries, 15s interval)
#   kubectl_retry 10 30 kubectl_args...  (uses 10 retries, 30s interval)
function kubectl_retry() {
	local max_tries=5
	local interval=15

	# Check if first two arguments are numbers (for max_tries and interval)
	if [[ "${1}" =~ ^[0-9]+$ ]] && [[ "${2}" =~ ^[0-9]+$ ]]; then
		max_tries="${1}"
		interval="${2}"
		shift 2
	fi

	local i=0
	while true; do
		if kubectl "$@"; then
			return 0
		fi
		i=$((i + 1))
		if [[ "${i}" -lt "${max_tries}" ]]; then
			echo "'kubectl $*' failed, retrying in ${interval} seconds" 1>&2
		else
			break
		fi
		sleep "${interval}"
	done
	echo "'kubectl $*' failed after ${max_tries} tries" 1>&2 && return 1
}

function waitForProcess() {
	wait_time="$1"
	sleep_time="$2"
	cmd="$3"
	while [[ "${wait_time}" -gt 0 ]]; do
		if eval "${cmd}"; then
			return 0
		else
			sleep "${sleep_time}"
			wait_time=$((wait_time-sleep_time))
		fi
	done
	return 1
}

function waitForCmdWithAbortCmd() {
	wait_time="$1"
	sleep_time="$2"
	cmd="$3"
	abort_cmd="$4"
	while [[ "${wait_time}" -gt 0 ]]; do
		if eval "${cmd}"; then
			return 0
		elif eval "${abort_cmd}"; then
			return 1
		else
			sleep "${sleep_time}"
			wait_time=$((wait_time-sleep_time))
		fi
	done
	return 1
}

# Check if the $1 argument is the name of a 'known'
# Kata runtime. Of course, the end user can choose any name they
# want in reality, but this function knows the names of the default
# and recommended Kata docker runtime install names.
function is_a_kata_runtime() {
	if [[ "$1" = "containerd-shim-kata-v2" ]] || [[ "$1" = "io.containerd.kata.v2" ]]; then
		echo "1"
	else
		echo "0"
	fi
}

# Gets versions and paths of all the components
# list in kata-env
function extract_kata_env() {
	local cmd
	local config_path
	local runtime_version
	local runtime_version_semver
	local runtime_version_commit
	local runtime_path
	local hypervisor_path
	# shellcheck disable=SC2034
	local virtiofsd_path
	local initrd_path
	local kata_env
	# shellcheck disable=SC2034
	local req_memory_amount
	# shellcheck disable=SC2034
	local req_num_vcpus

	case "${KATA_HYPERVISOR}" in
		dragonball)
			cmd=kata-ctl
			config_path=".runtime.config.path"
			runtime_version=".runtime.version"
			runtime_version_semver="semver"
			runtime_version_commit="commit"
			runtime_path=".runtime.path"
			hypervisor_path=".hypervisor.path"
			virtio_fs_daemon_path=".hypervisor.virtio_fs_daemon"
			initrd_path=".initrd.path"
			shared_fs=".hypervisor.shared_fs"
			;;
		*)
			cmd=kata-runtime
			config_path=".Runtime.Config.Path"
			runtime_version=".Runtime.Version"
			runtime_version_semver="Semver"
			runtime_version_commit="Commit"
			runtime_path=".Runtime.Path"
			hypervisor_path=".Hypervisor.Path"
			virtio_fs_daemon_path=".Hypervisor.VirtioFSDaemon"
			initrd_path=".Initrd.Path"
			shared_fs=".Hypervisor.SharedFS"
			# shellcheck disable=SC2034
			req_memory_amount=".Host.Memory.Total"
			# shellcheck disable=SC2034
			req_num_vcpus=""
			;;
	esac
	kata_env="$(sudo "${cmd}" env --json)"

	RUNTIME_CONFIG_PATH="$(echo "${kata_env}" | jq -r "${config_path}")"
	RUNTIME_VERSION="$(echo "${kata_env}" | jq -r "${runtime_version}" | grep "${runtime_version_semver}" | cut -d'"' -f4)"
	# shellcheck disable=SC2034
	RUNTIME_COMMIT="$(echo "${kata_env}" | jq -r "${runtime_version}" | grep "${runtime_version_commit}" | cut -d'"' -f4)"
	RUNTIME_PATH="$(echo "${kata_env}" | jq -r "${runtime_path}")"
	# shellcheck disable=SC2034
	SHARED_FS="$(echo "${kata_env}" | jq -r "${shared_fs}")"

	# get the requested memory and num of vcpus from the kata config file.
	config_content="$(grep -vE "^#" "${RUNTIME_CONFIG_PATH}")"
	# shellcheck disable=SC2034
	REQ_MEMORY="$(echo "${config_content}" | grep -i 'default_memory =' | cut -d  "=" -f2 | awk '{print $1}')"
	# shellcheck disable=SC2034
	REQ_NUM_VCPUS="$(echo "${config_content}" | grep -i 'default_vcpus =' | cut -d  "=" -f2 | awk '{print $1}')"

	# Shimv2 path is being affected by https://github.com/kata-containers/kata-containers/issues/1151
	SHIM_PATH=$(command -v containerd-shim-kata-v2)
	[[ -L "${SHIM_PATH}" ]] && SHIM_PATH=$(readlink "${SHIM_PATH}")

	# shellcheck disable=SC2034
	SHIM_VERSION=${RUNTIME_VERSION}

	HYPERVISOR_PATH=$(echo "${kata_env}" | jq -r "${hypervisor_path}")
	# shellcheck disable=SC2034
	VIRTIOFSD_PATH=$(echo "${kata_env}" | jq -r "${virtio_fs_daemon_path}")
	# shellcheck disable=SC2034
	INITRD_PATH=$(echo "${kata_env}" | jq -r "${initrd_path}")

	# TODO: there is no ${cmd} of rust version currently
	if [[ "${KATA_HYPERVISOR}" != "dragonball" ]]; then
		if [[ "${KATA_HYPERVISOR}" = "stratovirt" ]]; then
			HYPERVISOR_VERSION=$(sudo -E "${HYPERVISOR_PATH}" -version | head -n1)
		else
			# shellcheck disable=SC2034
			HYPERVISOR_VERSION=$(sudo -E "${HYPERVISOR_PATH}" --version | head -n1)
		fi
	fi
}

# Checks that processes are not running
function check_processes() {
	extract_kata_env

	# Only check the kata-env if we have managed to find the kata executable...
	if [[ -x "${RUNTIME_PATH}" ]]; then
		local vsock_configured
		# shellcheck disable=SC2034
		vsock_configured=$(${RUNTIME_PATH} env | awk '/UseVSock/ {print $3}')
		local vsock_supported
		# shellcheck disable=SC2034
		vsock_supported=$(${RUNTIME_PATH} env | awk '/SupportVSock/ {print $3}')
	else
		# shellcheck disable=SC2034
		local vsock_configured="false"
		# shellcheck disable=SC2034
		local vsock_supported="false"
	fi

	general_processes=( "${HYPERVISOR_PATH}" "${SHIM_PATH}" )

	for i in "${general_processes[@]}"; do
		[[ -z "${i}" ]] && continue
		if pgrep -f "${i}"; then
			die "Found unexpected ${i} present"
		fi
	done
}

# Clean environment, this function will try to remove all
# stopped/running containers.
function clean_env()
{
	# If the timeout has not been set, default it to 30s
	# Docker has a built in 10s default timeout, so make ours
	# longer than that.
	KATA_DOCKER_TIMEOUT=${KATA_DOCKER_TIMEOUT:-30}
	containers_running=$(sudo timeout "${KATA_DOCKER_TIMEOUT}" docker ps -q)

	if [[ -n "${containers_running}" ]]; then
		# First stop all containers that are running
		# Use kill, as the containers are generally benign, and most
		# of the time our 'stop' request ends up doing a `kill` anyway
		# shellcheck disable=SC2086
		sudo timeout "${KATA_DOCKER_TIMEOUT}" docker kill ${containers_running}

		# Remove all containers
		# shellcheck disable=SC2046
		sudo timeout "${KATA_DOCKER_TIMEOUT}" docker rm -f $(docker ps -qa)
	fi
}

function clean_env_ctr()
{
	local count_running
	count_running="$(sudo ctr c list -q | wc -l)"
	local remaining_attempts=10
	declare -a running_tasks=()
	local count_tasks=0
	local sleep_time=1
	local time_out=10

	[[ "${count_running}" -eq "0" ]] && return 0

	readarray -t running_tasks < <(sudo ctr t list -q)

	info "Wait until the containers gets removed"

	for task_id in "${running_tasks[@]}"; do
		sudo timeout -s SIGKILL 30s ctr t kill -a -s SIGKILL "${task_id}" >/dev/null 2>&1 || true
		sleep 0.5
	done

	# do not stop if the command fails, it will be evaluated by waitForProcess
	local cmd
	cmd="[[ \$(sudo ctr tasks list | grep -c \"STOPPED\" || true) == \"${count_running}\" ]]" || true

	local res="ok"
	waitForProcess "${time_out}" "${sleep_time}" "${cmd}" || res="fail"

	[[ "${res}" == "ok" ]] || sudo systemctl restart containerd

	while (( remaining_attempts > 0 )); do
		# shellcheck disable=SC2046
		[[ "${RUNTIME}" == "runc" ]] && sudo ctr tasks rm -f $(sudo ctr task list -q)
		# shellcheck disable=SC2046
		sudo ctr c rm $(sudo ctr c list -q) >/dev/null 2>&1

		count_running="$(sudo ctr c list -q | wc -l)"

		[[ "${count_running}" -eq 0 ]] && break

		remaining_attempts=$((remaining_attempts-1))
		sleep 0.5
	done

	count_tasks="$(sudo ctr t list -q | wc -l)"

	if (( count_tasks > 0 )); then
		die "Can't remove running containers."
	fi
}

# Restarts a systemd service while ensuring the start-limit-burst is set to 0.
# Outputs warnings to stdio if something has gone wrong.
#
# Returns 0 on success, 1 otherwise
function restart_systemd_service_with_no_burst_limit() {
	local service=$1
	info "restart ${service} service"

	local active
	active=$(systemctl show "${service}.service" -p ActiveState | cut -d'=' -f2) || true
	[[ "${active}" == "active" ]] || warn "Service ${service} is not active"

	local start_burst
	start_burst=$(systemctl show "${service}".service -p StartLimitBurst | cut -d'=' -f2) || true
	if [[ "${start_burst}" -ne 0 ]]
	then
		local unit_file
		unit_file=$(systemctl show "${service}.service" -p FragmentPath | cut -d'=' -f2) || true
		[[ -f "${unit_file}" ]] || { warn "Can't find ${service}'s unit file: ${unit_file}"; return 1; }

		# If the unit file is in /lib or /usr/lib, copy it to /etc
		if [[ ${unit_file} =~ ^/(usr/)?lib/ ]]; then
			tmp_unit_file="/etc/${unit_file#*lib/}"
			sudo cp "${unit_file}" "${tmp_unit_file}"
			unit_file="${tmp_unit_file}"
		fi

		local start_burst_set
		start_burst_set=$(sudo grep StartLimitBurst "${unit_file}" | wc -l) || true
		if [[ "${start_burst_set}" -eq 0 ]]
		then
			sudo sed -i '/\[Service\]/a StartLimitBurst=0' "${unit_file}"
		else
			sudo sed -i 's/StartLimitBurst.*$/StartLimitBurst=0/g' "${unit_file}"
		fi

		sudo systemctl daemon-reload
	fi

	sudo systemctl restart "${service}" || true

	local state
	state=$(systemctl show "${service}.service" -p SubState | cut -d'=' -f2) || true
	if [[ "${state}" != "running" ]]; then
		warn "Can't restart the ${service} service (SubState=${state})"
		warn "journalctl output for ${service}:"
		sudo journalctl -xeu "${service}.service" --no-pager -n 50 || true
		return 1
	fi

	start_burst=$(systemctl show "${service}.service" -p StartLimitBurst | cut -d'=' -f2) || true
	[[ "${start_burst}" -eq 0 ]] || { warn "Can't set start burst limit for ${service} service"; return 1; }

	return 0
}

function restart_containerd_service() {
	restart_systemd_service_with_no_burst_limit containerd || return 1

	local retries=5
	local counter=0
	until [[ "${counter}" -ge "${retries}" ]] || sudo ctr --connect-timeout 1s version > /dev/null 2>&1
	do
		info "Waiting for containerd socket..."
		((counter++))
	done

	if [[ "${counter}" -ge "${retries}" ]]; then
		warn "Can't connect to containerd socket after ${retries} retries"
		warn "journalctl output for containerd:"
		sudo journalctl -xeu containerd.service --no-pager -n 50 || true
		return 1
	fi

	clean_env_ctr
	return 0
}

function restart_crio_service() {
	sudo systemctl restart crio
}

# Extracts numeric schema from a config blob (effective or file content). Returns 0 when missing/invalid.
function _containerd_blob_schema_version() {
	local line val
	line="$(grep -m1 -E '^[[:space:]]*version[[:space:]]*=' <<< "${1:-}" 2>/dev/null || true)"
	val="$(sed -e 's/^[[:space:]]*version[[:space:]]*=[[:space:]]*//' -e 's/[[:space:]]*\(#.*\)\?$//' <<< "${line}")"
	val="${val//\'/}"
	val="${val//\"/}"
	val="${val//[[:space:]]/}"

	[[ "${val}" =~ ^[0-9]+$ ]] || { echo "0" && return; }
	echo "${val}"
}

# Reads numeric schema version from a containerd config file (leading "version = N" line).
function _containerd_config_schema_version() {
	local cfg="${1:?}"

	[[ ! -f "${cfg}" ]] && echo "0" && return
	_containerd_blob_schema_version "$(cat "${cfg}" 2>/dev/null || sudo cat "${cfg}" 2>/dev/null || true)"
}

# Requires merged effective config (preferred) or main config file to use schema >= 3.
function require_containerd_config_schema_v3_plus() {
	local dump schema

	dump="$(PATH="${PATH}:/usr/local/bin:/usr/local/sbin" containerd config dump 2>/dev/null || true)"
	if [[ -n "${dump}" ]]; then
		schema="$(_containerd_blob_schema_version "${dump}")"
	else
		schema="$(_containerd_config_schema_version "/etc/containerd/config.toml")"
	fi

	[[ "${schema}" =~ ^[0-9]+$ ]] || die "containerd: could not determine config schema version (expected >= 3)"
	[[ "${schema}" -ge 3 ]] || die "containerd: config schema version ${schema} is not supported; require version >= 3 (refusing legacy v1/v2)"
}

# Requires the installed containerd's default config to use schema >= 3 (containerd 2.x).
function require_containerd_binary_default_schema_v3_plus() {
	local blob schema

	blob="$(PATH="${PATH}:/usr/local/bin:/usr/local/sbin" containerd config default 2>/dev/null || true)"
	schema="$(_containerd_blob_schema_version "${blob}")"
	[[ "${schema}" =~ ^[0-9]+$ ]] || die "containerd: could not read schema from config default (expected >= 3)"
	[[ "${schema}" -ge 3 ]] || die "containerd defaults to config schema version ${schema}; these tests require containerd 2.x (schema >= 3)"
}

# Effective config schema: on-disk main (/etc/containerd/config.toml), else merged dump,
# else binary config default (used to pick [grpc]/[ttrpc] vs server plugin layout).
function _containerd_resolved_schema_version() {
	local schema cdbin

	cdbin="$(PATH="${PATH}:/usr/local/bin:/usr/local/sbin" command -v containerd || true)"
	[[ -n "${cdbin}" ]] || { echo "0"; return 0; }

	schema="$(_containerd_config_schema_version "/etc/containerd/config.toml")"
	if [[ "${schema}" =~ ^[0-9]+$ ]] && [[ "${schema}" -ge 3 ]]; then
		echo "${schema}"
		return 0
	fi

	schema="$(_containerd_blob_schema_version "$("${cdbin}" config dump 2>/dev/null || true)")"
	if [[ "${schema}" =~ ^[0-9]+$ ]] && [[ "${schema}" -ge 3 ]]; then
		echo "${schema}"
		return 0
	fi

	schema="$(_containerd_blob_schema_version "$("${cdbin}" config default 2>/dev/null || true)")"
	if [[ "${schema}" =~ ^[0-9]+$ ]]; then
		echo "${schema}"
		return 0
	fi

	echo "0"
}

# Emit TOML to stdout: force uid/gid 0 on API sockets for this config schema ($1).
# Schema v3 uses top-level [grpc]/[ttrpc]; v4+ uses io.containerd.server.v1.* plugins (see containerd-config.toml.5).
function containerd_emit_rootful_api_socket_overrides() {
	local schema="${1:?schema argument required}"

	if [[ "${schema}" =~ ^[0-9]+$ ]] && [[ "${schema}" -ge 4 ]]; then
		cat <<'EOF'
[plugins.'io.containerd.server.v1.grpc']
  uid = 0
  gid = 0

[plugins.'io.containerd.server.v1.ttrpc']
  uid = 0
  gid = 0
EOF
	else
		cat <<'EOF'
[grpc]
  uid = 0
  gid = 0

[ttrpc]
  uid = 0
  gid = 0
EOF
	fi
}

# Rootful systemd must own API sockets (see containerd "config default" using non-root
# uid/gid under listeners on newer releases, e.g. 2.3 on amd64).
#
# Only containerd 2.x (schema v3+) emits a non-root uid/gid in "config default" and
# honours conf.d drop-ins, so the override is written as a conf.d fragment there.
# containerd 1.x (schema v2) already uses root-owned API sockets and does not honour
# conf.d the same way, so there is nothing to do.
function ensure_containerd_conf_d_rootful_api_sockets() {
	local drop_in="/etc/containerd/conf.d/99-kata-ci-rootful-api-sockets.toml"
	local schema

	schema="$(_containerd_resolved_schema_version)"
	[[ "${schema}" -ge 3 ]] || return 0

	sudo mkdir -p "$(dirname "${drop_in}")"
	containerd_emit_rootful_api_socket_overrides "${schema}" | sudo tee "${drop_in}" >/dev/null
}

# Writes containerd's config default to $1, replacing the imports line so fragments load from $2.
function containerd_render_config_default_with_imports() {
	local out="$1"
	local abs_conf_d="$2"
	local cd_bin imp_line

	abs_conf_d="${abs_conf_d%/}"
	cd_bin="$(PATH="${PATH}:/usr/local/bin:/usr/local/sbin" command -v containerd)"
	[[ -n "${cd_bin}" ]] || die "containerd not found in PATH"

	imp_line="imports = [\"${abs_conf_d}/*.toml\"]"

	"${cd_bin}" config default | awk -v imp="${imp_line}" '
		/^imports[[:space:]]*=/ && !did { print imp; did=1; next }
		{ print }
		END {
			if (!did) {
				print "containerd_render_config_default_with_imports: no imports= line in config default" > "/dev/stderr"
				exit 2
			}
		}
	' >"${out}"
}

# Configures containerd for CI; handles schema v2 (containerd v1.x) and v3+ (containerd v2.x).
#
# containerd 2.x (schema v3+) loads conf.d drop-in fragments, so the base config is
# regenerated from "containerd config default" (which already imports conf.d) and the
# Kata runtime / rootful-socket overrides are written there.  containerd 1.x (schema
# v2) does not honour conf.d the same way, so its config.toml is replaced wholesale
# with a complete, self-contained file.
function overwrite_containerd_config() {
	local containerd_config="/etc/containerd/config.toml"
	local conf_dir drop_in hv cfg_path shim_binary schema cd_bin runc_path

	conf_dir="$(dirname "${containerd_config}")/conf.d"
	drop_in="${conf_dir}/50-kata-containers-ci.toml"

	schema="$(_containerd_resolved_schema_version)"
	hv="${KATA_HYPERVISOR:-qemu}"
	cfg_path="${KATA_CONFIG_PATH:-/opt/kata/share/defaults/kata-containers/configuration-${hv}.toml}"
	shim_binary="$(command -v "containerd-shim-kata-${hv}-v2" 2>/dev/null || true)"
	[[ -n "${shim_binary}" ]] || shim_binary="/usr/local/bin/containerd-shim-kata-${hv}-v2"

	sudo mkdir -p "$(dirname "${containerd_config}")"

	if [[ "${schema}" -ge 3 ]]; then
		# Always regenerate from the installed binary so the schema version and
		# all fields match exactly what this containerd binary expects.  Keeping a
		# stale config.toml from a different containerd version causes MigrateConfigTo
		# to panic on schema mismatches (e.g. a config with version=3 loaded by an
		# older binary whose migrations slice only covers versions 0-2).
		info "Regenerating ${containerd_config} from containerd config default"
		cd_bin="$(command -v containerd)"
		sudo mkdir -p "${conf_dir}"
		sudo "${cd_bin}" config default | sudo tee "${containerd_config}" > /dev/null
		ensure_containerd_conf_d_rootful_api_sockets

		# containerd v2.x (schema v3+): io.containerd.cri.v1.runtime plugin path,
		# written as a conf.d drop-in fragment.
		sudo tee "${drop_in}" >/dev/null << EOF
[plugins.'io.containerd.cri.v1.runtime'.containerd]
  default_runtime_name = 'kata'

[plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes.kata]
  runtime_type = 'io.containerd.kata-${hv}.v2'
  sandboxer = 'podsandbox'

  [plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes.kata.options]
    ConfigPath = '${cfg_path}'
    BinaryName = '${shim_binary}'
EOF
	else
		# containerd v1.x (schema v2): conf.d drop-ins are not honoured the same
		# way, so replace config.toml wholesale with a complete, self-contained
		# file.  The v1.x default API sockets are already root-owned, so no socket
		# override is required.
		info "Writing complete ${containerd_config} for containerd v1.x (schema v2)"
		runc_path="$(command -v runc || echo /usr/bin/runc)"
		sudo tee "${containerd_config}" >/dev/null << EOF
version = 2

[plugins."io.containerd.grpc.v1.cri".containerd]
  default_runtime_name = "kata"

  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata]
    runtime_type = "io.containerd.kata-${hv}.v2"

    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata.options]
      ConfigPath = "${cfg_path}"
      BinaryName = "${shim_binary}"

  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc]
    runtime_type = "io.containerd.runc.v2"

    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc.options]
      BinaryName = "${runc_path}"
      SystemdCgroup = true
EOF
	fi
}

# Configures CRI-O
function overwrite_crio_config() {
	crio_conf_d="/etc/crio/crio.conf.d"
	sudo mkdir -p "${crio_conf_d}"

	kata_config="${crio_conf_d}/99-kata-containers"
	sudo tee "${kata_config}" << EOF
[crio.runtime.runtimes.kata]
runtime_path = "/usr/local/bin/containerd-shim-kata-v2"
runtime_type = "vm"
runtime_root = "/run/vc"
runtime_config_path = "/opt/kata/share/defaults/kata-containers/configuration.toml"
privileged_without_host_devices = true
EOF

	debug_config="${crio_conf_d}/100-debug"
	sudo tee "${debug_config}" << EOF
[crio]
log_level = "debug"
EOF
}

function install_tarball() {
	declare -r installed_dir="${1}"
	declare -r tarball_dir="${2}"
	declare -r tarball="${3}"
	declare -r remove_tarball_dir="${4}"
	declare -r destdir="/"

	if [[ "${remove_tarball_dir}" == "true" ]]; then
		# Removing previous tarball installation
		sudo rm -rf "${installed_dir}"
	fi

	pushd "${tarball_dir}" || return
	sudo tar --zstd -xvf "${tarball}" -C "${destdir}"
	popd || return
}

function install_kata_tools() {
	declare -r katadir="/opt/kata"
	declare -r tarballdir="${1:-kata-tools-artifacts}"
	declare -r local_bin_dir="/usr/local/bin/"

	install_tarball "${katadir}" "${tarballdir}" "kata-tools-static.tar.zst" false

	# create symbolic links to kata-tools components
	for b in "${katadir}"/bin/* ; do
		sudo ln -sf "${b}" "${local_bin_dir}/$(basename "${b}")"
	done
}

function install_kata() {
	declare -r katadir="/opt/kata"
	declare -r tarballdir="kata-artifacts"
	declare -r local_bin_dir="/usr/local/bin/"

	install_tarball "${katadir}" "${tarballdir}" "kata-static.tar.zst" true

	# create symbolic links to kata components
	for b in "${katadir}"/bin/* ; do
		sudo ln -sf "${b}" "${local_bin_dir}/$(basename "${b}")"
	done

	if [[ "${CONTAINER_ENGINE:=containerd}" = "containerd" ]]; then
		check_containerd_config_for_kata
		restart_containerd_service
	else
		overwrite_crio_config
		restart_crio_service
	fi

	load_vhost_mods
}

# creates a new kata configuration.toml hard link that
# points to the hypervisor passed by KATA_HYPERVISOR env var.
function enabling_hypervisor() {
	declare -r KATA_DIR="/opt/kata"
	declare -r CONTAINERD_SHIM_KATA="/usr/local/bin/containerd-shim-kata-${KATA_HYPERVISOR}-v2"

	case "${KATA_HYPERVISOR}" in
		dragonball|*-runtime-rs)
			sudo ln -sf "${KATA_DIR}/runtime-rs/bin/containerd-shim-kata-v2" "${CONTAINERD_SHIM_KATA}"
			declare -r CONFIG_DIR="${KATA_DIR}/share/defaults/kata-containers/runtime-rs"
			;;
		*)
			sudo ln -sf "${KATA_DIR}/bin/containerd-shim-kata-v2" "${CONTAINERD_SHIM_KATA}"
			declare -r CONFIG_DIR="${KATA_DIR}/share/defaults/kata-containers"
			;;
	esac

	declare -r SRC_HYPERVISOR_CONFIG="${CONFIG_DIR}/configuration-${KATA_HYPERVISOR}.toml"
	declare -r DEST_KATA_CONFIG="${CONFIG_DIR}/configuration.toml"

	sudo ln -sf "${SRC_HYPERVISOR_CONFIG}" "${DEST_KATA_CONFIG}"

	export KATA_CONFIG_PATH="${DEST_KATA_CONFIG}"
}


function check_containerd_config_for_kata() {
	declare -r containerd_path="/etc/containerd/config.toml"
	local hv dump

	hv="${KATA_HYPERVISOR:-qemu}"

	dump="$(PATH="${PATH}:/usr/local/bin:/usr/local/sbin" containerd config dump 2>/dev/null || true)"

	if [[ -z "${dump}" ]] && [[ -f "${containerd_path}" ]]; then
		dump="$(sudo cat "${containerd_path}")"
	fi

	if echo "${dump}" | grep -qE "default_runtime_name[[:space:]]*=[[:space:]]*[\"']kata[\"']" && \
		echo "${dump}" | grep -qE "runtime_type[[:space:]]*=[[:space:]]*[\"']io\\.containerd\\.kata(-${hv})?\\.v2[\"']"; then
		info "containerd ok"
	else
		info "writing Kata overrides for containerd (current schema from containerd config default)"
		overwrite_containerd_config
	fi
}

function ensure_yq() {
	: "${GOPATH:=${GITHUB_WORKSPACE:-${HOME}/go}}"
	export GOPATH
	export PATH="${GOPATH}/bin:${PATH}"
	INSTALL_IN_GOPATH=true "${repo_root_dir}/ci/install_yq.sh"
	hash -d yq 2> /dev/null || true # yq is preinstalled on GHA Ubuntu 22.04 runners so we clear Bash's PATH cache.
}

function ensure_pip() {
	command -v python3 &> /dev/null || die "python3 is required"
	python3 -m pip --version &> /dev/null && return

	python3 -m ensurepip --user 2>/dev/null || \
		(sudo apt-get update && \
			sudo apt-get install -y python3-pip 2>/dev/null) || \
		die "failed to bootstrap pip"

	python3 -m pip --version &> /dev/null || die "pip is unavailable after bootstrap"
}

function install_tomlq() {
	command -v jq &> /dev/null || die "jq is required by tomlq but was not found"
	command -v tomlq &> /dev/null && echo "tomlq is already installed." && return
	ensure_pip

	echo "tomlq is not installed. Installing..."
	python3 -m pip install --user --upgrade yq tomlkit 2>/dev/null || \
		python3 -m pip install --user --upgrade --break-system-packages yq tomlkit || \
		die "failed to install tomlq"

	# Save the original PATH before modifying it
	export _TOMLQ_ORIGINAL_PATH="${PATH}"
	export PATH="${HOME}/.local/bin:${PATH}"
	hash -r

	if command -v tomlq &> /dev/null; then
		export _TOMLQ_INSTALLED=true
	else
		die "tomlq installation failed"
	fi
}

function uninstall_tomlq() {
	if [[ -z "${_TOMLQ_INSTALLED:-}" ]]; then
		echo "tomlq was not installed by install_tomlq(); skipping uninstall."
		return
	fi

	if command -v tomlq &> /dev/null; then
		echo "Uninstalling tomlq..."

		# Only attempt uninstall if python3 and pip are available
		if command -v python3 &> /dev/null && python3 -m pip --version &> /dev/null; then
			python3 -m pip uninstall -y yq tomlkit 2>/dev/null || \
				python3 -m pip uninstall -y --break-system-packages yq tomlkit || \
				die "failed to uninstall tomlq"
		else
			warn "tomlq found in PATH but python3 or pip unavailable; skipping uninstall (likely preinstalled)"
		fi
	fi

	# Restore the original PATH if it was saved by install_tomlq
	if [[ -n "${_TOMLQ_ORIGINAL_PATH}" ]]; then
		export PATH="${_TOMLQ_ORIGINAL_PATH}"
		unset _TOMLQ_ORIGINAL_PATH
		hash -r
	fi
}

function ensure_helm() {
	ensure_yq
	# The get-helm-3 script will take care of downloaading and installing Helm
	# properly on the system respecting ARCH, OS and other configurations.
	DESIRED_VERSION=$(get_from_kata_deps ".externals.helm.version")
	export DESIRED_VERSION

	# Check if helm is available in the system's PATH
	if ! command -v helm &> /dev/null; then
		echo "Helm is not installed. Installing Helm..."
		curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
		# Verify the installation
		if command -v helm &> /dev/null; then
			echo "Helm installed successfully."
		else
			echo "Failed to install Helm."
			exit 1
		fi
	else
		echo "Helm is already installed."
	fi
}

# dependency: What we want to get the version from the versions.yaml file
function get_from_kata_deps() {
        versions_file="${repo_root_dir}/versions.yaml"

        # shellcheck disable=SC2016
        command -v yq &>/dev/null || die 'yq command is not in your $PATH'

        yq_version=$(yq --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | cut -d. -f1)
        if [[ "${yq_version}" -eq 3 ]]; then
          dependency="${1#.}"
          result=$("yq" read "${versions_file}" "${dependency}")
        else
          dependency=$1
          result=$("yq" "${dependency} | explode (.)" "${versions_file}")
        fi

        [[ "${result}" = "null" ]] && result=""
        echo "${result}"
}

# project: org/repo format
# base_version: ${major}.${minor}
# allow_unstable: Whether alpha / beta releases should be considered (default: false)
function get_latest_patch_release_from_a_github_project() {
        project="${1}"
        base_version="${2}"
        allow_unstable="${3:-false}"

        regex="^${base_version}.[0-9]*$"
        if [[ "${allow_unstable}" == "true" ]]; then
                regex="^${base_version}.[0-9]*"
        fi

        curl \
          ${GH_TOKEN:+--header "Authorization: Bearer ${GH_TOKEN:-}"} \
          --fail-with-body \
          --show-error \
          --silent \
          "https://api.github.com/repos/${project}/releases" \
          | jq -r .[].tag_name \
          | grep "${regex}" -m1
}

# GitHub Actions' setup-go often sets GOTOOLCHAIN=local, which forbids fetching a newer
# toolchain required by cloned containerd (e.g. v2.3 go.mod vs Kata's pinned Go). Use
# automatic toolchain selection only while building upstream containerd.
function export_go_toolchain_for_containerd_source_builds() {
	export GOTOOLCHAIN=auto
	info "GOTOOLCHAIN=auto so containerd is built with the toolchain its go.mod requires"
}

# base_version: The version to be intalled in the ${major}.${minor} format
function clone_cri_containerd() {
	base_version="${1}"

	project="containerd/containerd"
	version=$(get_latest_patch_release_from_a_github_project "${project}" "${base_version}")

	rm -rf containerd
	git clone -b "${version}" "https://github.com/${project}"
}

# project: org/repo format
# version: the version of the tarball that will be downloaded
# tarball-name: the name of the tarball that will be downloaded
function download_github_project_tarball() {
	project="${1}"
	version="${2}"
	tarball_name="${3}"

	wget ${GH_TOKEN:+--header="Authorization: Bearer ${GH_TOKEN}"} \
		"https://github.com/${project}/releases/download/${version}/${tarball_name}"
}

# version: The version to be intalled
function install_cni_plugins() {
	version="${1}"

	project="containernetworking/plugins"
	tarball_name="cni-plugins-linux-$("${repo_root_dir}"/tests/kata-arch.sh -g)-${version}.tgz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	sudo mkdir -p /opt/cni/bin
	sudo tar -xvf "${tarball_name}" -C /opt/cni/bin
	rm -f "${tarball_name}"

	cni_config="/etc/cni/net.d/10-containerd-net.conflist"
	if [[ ! -f "${cni_config}" ]];then
		sudo mkdir -p /etc/cni/net.d
		sudo tee "${cni_config}" << EOF
{
  "cniVersion": "1.0.0",
  "name": "containerd-net",
  "plugins": [
    {
      "type": "bridge",
      "bridge": "cni0",
      "isGateway": true,
      "ipMasq": true,
      "promiscMode": true,
      "ipam": {
        "type": "host-local",
        "ranges": [
          [{
            "subnet": "10.88.0.0/16"
          }],
          [{
            "subnet": "2001:4860:4860::/64"
          }]
        ],
        "routes": [
          { "dst": "0.0.0.0/0" },
          { "dst": "::/0" }
        ]
      }
    },
    {
      "type": "portmap",
      "capabilities": {"portMappings": true}
    }
  ]
}
EOF
	fi
}

# version: The version to be installed
function install_runc() {
	base_version="${1}"
	project="opencontainers/runc"
	version=$(get_latest_patch_release_from_a_github_project "${project}" "${base_version}")

	if [[ -f /usr/local/sbin/runc ]]; then
		return
	fi

	binary_name="runc.$("${repo_root_dir}"/tests/kata-arch.sh -g)"
	download_github_project_tarball "${project}" "${version}" "${binary_name}"

	sudo mkdir -p /usr/local/sbin
	sudo mv "${binary_name}" /usr/local/sbin/runc
	sudo chmod +x /usr/local/sbin/runc
}

# base_version: The version to be intalled in the ${major}.${minor} format
function install_cri_containerd() {
	base_version="${1}"

	project="containerd/containerd"
	version=$(get_latest_patch_release_from_a_github_project "${project}" "${base_version}" "true")

	tarball_name="containerd-${version//v}-linux-$("${repo_root_dir}"/tests/kata-arch.sh -g).tar.gz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	#add the "--keep-directory-symlink" option to make sure the untar wouldn't override the
	#system rootfs's bin/sbin directory which would be a symbol link to /usr/bin or /usr/sbin.
	if [[ ! -f /usr/local ]]; then
		sudo mkdir -p /usr/local
	fi
	sudo tar --keep-directory-symlink -xvf "${tarball_name}" -C /usr/local/
	rm -f "${tarball_name}"

	sudo mkdir -p /etc/containerd
	sudo containerd config default \
		| sed -E 's/^([[:space:]]*SystemdCgroup[[:space:]]*=[[:space:]]*)false/\1true/' \
		| sudo tee /etc/containerd/config.toml > /dev/null
	ensure_containerd_conf_d_rootful_api_sockets

	# Drop a default /etc/crictl.yaml pointing at the freshly-installed
	# containerd socket so crictl does not probe — and warn loudly about
	# — the legacy default endpoints (dockershim, CRI-O, cri-dockerd) on
	# every invocation. cri-tools v1.30+ deprecated the implicit default
	# endpoint discovery, which means every crictl call without this
	# config emits noisy validation errors for sockets that do not exist
	# on the runner.
	sudo tee /etc/crictl.yaml > /dev/null <<-EOF
		runtime-endpoint: unix:///run/containerd/containerd.sock
		image-endpoint: unix:///run/containerd/containerd.sock
		timeout: 10
	EOF

	# Always write the service file pointing at the just-installed binary and
	# reload systemd so the correct binary is used on the next start.
	# The runner image may have a pre-installed containerd unit pointing at a
	# different (older) binary; leaving that in place causes systemd to start
	# the wrong binary with a config it cannot parse, leading to a panic in
	# MigrateConfigTo (index out of range because the old binary's migrations
	# slice is shorter than the config schema version requires).
	containerd_service="/etc/systemd/system/containerd.service"
	sudo mkdir -p /etc/systemd/system
	sudo tee "${containerd_service}" > /dev/null <<EOF
[Unit]
Description=containerd container runtime
Documentation=https://containerd.io
After=network.target local-fs.target

[Service]
ExecStartPre=-/sbin/modprobe overlay
ExecStart=/usr/local/bin/containerd

Type=notify
Delegate=yes
KillMode=process
Restart=always
RestartSec=5
# Having non-zero Limit*s causes performance problems due to accounting overhead
# in the kernel. We recommend using cgroups to do container-local accounting.
LimitNPROC=infinity
LimitCORE=infinity
LimitNOFILE=infinity
# Comment TasksMax if your systemd version does not supports it.
# Only systemd 226 and above support this version.
TasksMax=infinity
OOMScoreAdjust=-999

[Install]
WantedBy=multi-user.target
EOF
	sudo systemctl daemon-reload
}

# Installs cri-tools (crictl). When a base_version (${major}.${minor}) is
# supplied the matching latest patch release is used; otherwise — and this is
# the default in CI — the absolute latest stable release published on GitHub
# is fetched. cri-tools is intentionally not pinned in versions.yaml so we
# always exercise a crictl that speaks current CRI protocol revisions.
function install_cri_tools() {
	base_version="${1:-}"

	project="kubernetes-sigs/cri-tools"
	if [[ -n "${base_version}" ]]; then
		version=$(get_latest_patch_release_from_a_github_project "${project}" "${base_version}")
	else
		version=$(curl \
			${GH_TOKEN:+--header "Authorization: Bearer ${GH_TOKEN}"} \
			--fail-with-body \
			--show-error \
			--silent \
			"https://api.github.com/repos/${project}/releases/latest" \
			| jq -r .tag_name)
	fi

	tarball_name="crictl-${version}-linux-$("${repo_root_dir}"/tests/kata-arch.sh -g).tar.gz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	sudo tar -xvf "${tarball_name}" -C /usr/local/bin
	rm -f "${tarball_name}"
}

function install_nydus() {
	version="${1}"

	project="dragonflyoss/image-service"
	tarball_name="nydus-static-${version}-linux-$("${repo_root_dir}"/tests/kata-arch.sh -g).tgz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	sudo tar xfz "${tarball_name}" -C /usr/local/bin --strip-components=1
	rm -f "${tarball_name}"
}

function install_nydus_snapshotter() {
	version="${1}"

	project="containerd/nydus-snapshotter"
	tarball_name="nydus-snapshotter-${version}-$(uname -s| tr '[:upper:]' '[:lower:]')-$("${repo_root_dir}"/tests/kata-arch.sh -g).tar.gz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	sudo tar xfz "${tarball_name}" -C /usr/local/bin --strip-components=1
	rm -f "${tarball_name}"
}

# version: the CRI-O version to be installe
function install_crio() {
	local version=${1}

	sudo mkdir -p /etc/apt/keyrings
	sudo mkdir -p /etc/apt/sources.list.d

	curl -fsSL "https://pkgs.k8s.io/addons:/cri-o:/stable:/v${version}/deb/Release.key" | \
		sudo gpg --dearmor -o /etc/apt/keyrings/cri-o-apt-keyring.gpg
	echo "deb [signed-by=/etc/apt/keyrings/cri-o-apt-keyring.gpg] https://pkgs.k8s.io/addons:/cri-o:/stable:/v${version}/deb/ /" | \
		sudo tee /etc/apt/sources.list.d/cri-o.list

	sudo apt update
	sudo apt install -y cri-o

	# We need to set the default capabilities to ensure our tests will pass
	# See: https://github.com/kata-containers/kata-containers/issues/8034
	sudo mkdir -p /etc/crio/crio.conf.d/
	cat <<EOF | sudo tee /etc/crio/crio.conf.d/00-default-capabilities
[crio]
storage_option = [
	"overlay.skip_mount_home=true",
]
[crio.runtime]
default_capabilities = [
       "CHOWN",
       "DAC_OVERRIDE",
       "FSETID",
       "FOWNER",
       "SETGID",
       "SETUID",
       "SETPCAP",
       "NET_BIND_SERVICE",
       "KILL",
       "SYS_CHROOT",
]
EOF

	sudo systemctl enable --now crio
}

function install_docker() {
	# Add Docker's official GPG key
	sudo apt-get update
	sudo apt-get -y install ca-certificates curl gnupg
	sudo install -m 0755 -d /etc/apt/keyrings
	curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
	sudo chmod a+r /etc/apt/keyrings/docker.gpg

	# Add the repository to Apt sources:
	echo \
		"deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
		$(. /etc/os-release && echo "${VERSION_CODENAME}") stable" | \
		sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
	sudo apt-get update

	sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
}

# Convert architecture to the name used by golang
function arch_to_golang() {
	local -r arch="$(uname -m)"

	case "${arch}" in
		aarch64|arm64) echo "arm64";;
		ppc64le) echo "${arch}";;
		riscv64) echo "${arch}";;
		x86_64) echo "amd64";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: ${arch}";;
	esac
}

# Convert architecture to the name used by rust
function arch_to_rust() {
	local -r arch="$(uname -m)"

	case "${arch}" in
		aarch64|arm64) echo "aarch64";;
		ppc64le) echo "powerpc64le";;
		riscv64) echo "riscv64gc";;
		x86_64) echo "${arch}";;
		s390x) echo "${arch}";;
		*) die "unsupported architecture: ${arch}";;
	esac
}

# Convert architecture to the name used by the Linux kernel build system
function arch_to_kernel() {
	local -r arch="$(uname -m)"

	case "${arch}" in
		aarch64|arm64) echo "arm64";;
		ppc64le) echo "powerpc";;
		x86_64) echo "${arch}";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: ${arch}";;
	esac
}

# Obtain a list of the files the PR changed.
# Returns the information in format "${filter}\t${file}".
get_pr_changed_file_details_full()
{
        # List of filters used to restrict the types of file changes.
        # See git-diff-tree(1) for further info.
        local filters=""

        # Added file
        filters+="A"

        # Copied file
        filters+="C"

        # Modified file
        filters+="M"

        # Renamed file
        filters+="R"

        git diff-tree \
                -r \
                --name-status \
                --diff-filter="${filters}" \
                "origin/${branch}" HEAD
}

# Obtain a list of the files the PR changed, ignoring vendor files.
# Returns the information in format "${filter}\t${file}".
get_pr_changed_file_details()
{
        get_pr_changed_file_details_full | grep -v "vendor/"
}

function get_dep_from_yaml_db(){
        local versions_file="$1"
        local dependency="$2"

        [[ ! -f "${versions_file}" ]] && die "cannot find ${versions_file}"

        "${repo_root_dir}/ci/install_yq.sh" >&2

        result=$("${GOPATH}/bin/yq" "${dependency}" "${versions_file}")
        [[ "${result}" = "null" ]] && result=""
        echo "${result}"
}

function get_test_version(){
        local dependency="$1"

        local db

        # directory of this script, not the caller
        local cidir
        cidir=$(dirname "${BASH_SOURCE[0]}")

        db="${cidir}/../versions.yaml"

        get_dep_from_yaml_db "${db}" ".${dependency}"
}

# Load vhost, vhost_net, vhost_vsock modules.
function load_vhost_mods() {
	sudo modprobe vhost
	sudo modprobe vhost_net
	sudo modprobe vhost_vsock
}

function run_static_checks()
{
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	bash "${this_script_dir}/static-checks.sh" "$@"
}

function run_get_pr_changed_file_details()
{
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	get_pr_changed_file_details || true
}

# Check if the 1st argument version is greater than and equal to 2nd one
# Version format: [0-9]+ separated by period (e.g. 2.4.6, 1.11.3 and etc.)
#
# Parameters:
#	$1	- a version to be tested
#	$2	- a target version
#
# Return:
# 	0 if $1 is greater than and equal to $2
#	1 otherwise
function version_greater_than_equal() {
	local current_version=$1
	local target_version=$2
	smaller_version=$(echo -e "${current_version}\n${target_version}" | sort -V | head -1)
	if [[ "${smaller_version}" = "${target_version}" ]]; then
		return 0
	else
		return 1
	fi
}

# Run bats tests with proper reporting
#
# This function provides consistent test execution and reporting across
# all test suites (k8s, nvidia, kata-deploy, etc.)
#
# Parameters:
#	$1 - Test directory (where tests are located and reports will be saved)
#	$2 - Array name containing test files (passed by reference)
#
# Environment variables:
#	BATS_TEST_FAIL_FAST - Set to "yes" to stop at first failure (default: "no")
#
# Example usage:
#	tests=("test1.bats" "test2.bats")
#	run_bats_tests "/path/to/tests" tests
#
function run_bats_tests() {
	local test_dir="$1"
	local -n test_array=$2
	local fail_fast="${BATS_TEST_FAIL_FAST:-no}"

	local report_dir
	report_dir="${test_dir}/reports/$(date +'%F-%T')"
	mkdir -p "${report_dir}"

	info "Running tests with bats version: $(bats --version). Save outputs to ${report_dir}"

	local tests_fail=()
	for test_entry in "${test_array[@]}"; do
		test_entry=$(echo "${test_entry}" | tr -d '[:space:][:cntrl:]')
		[[ -z "${test_entry}" ]] && continue

		info "Executing ${test_entry}"

		# Output file will be prefixed with "ok" or "not_ok" based on the result
		local out_file="${report_dir}/${test_entry}.out"

		pushd "${test_dir}" > /dev/null || return
		if ! bats --timing --show-output-of-passing-tests "${test_entry}" | tee "${out_file}"; then
			tests_fail+=("${test_entry}")
			mv "${out_file}" "$(dirname "${out_file}")/not_ok-$(basename "${out_file}")"
			[[ "${fail_fast}" == "yes" ]] && break
		else
			mv "${out_file}" "$(dirname "${out_file}")/ok-$(basename "${out_file}")"
		fi
		popd > /dev/null || return
	done

	if [[ ${#tests_fail[@]} -ne 0 ]]; then
		die "Tests FAILED from suites: ${tests_fail[*]}"
	fi

	info "All tests SUCCEEDED"
}

# Report bats test results from the reports directory
#
# This function displays a summary of test results and outputs from
# the reports directory created by run_bats_tests().
#
# Parameters:
#	$1 - Test directory (where reports subdirectory is located)
#
# Example usage:
#	report_bats_tests "/path/to/tests"
#
function report_bats_tests() {
	local test_dir="$1"
	local reports_dir="${test_dir}/reports"

	if [[ ! -d "${reports_dir}" ]]; then
		warn "No reports directory found: ${reports_dir}"
		return 1
	fi

	for report_dir in "${reports_dir}"/*; do
		[[ ! -d "${report_dir}" ]] && continue

		local ok=()
		local not_ok=()
		mapfile -t ok < <(find "${report_dir}" -name "ok-*.out" 2>/dev/null)
		mapfile -t not_ok < <(find "${report_dir}" -name "not_ok-*.out" 2>/dev/null)

		cat <<-EOF
		SUMMARY ($(basename "${report_dir}")):
		 Pass:  ${#ok[*]}
		 Fail:  ${#not_ok[*]}
		EOF

		echo -e "\nSTATUSES:"
		for out in "${not_ok[@]}" "${ok[@]}"; do
			[[ -z "${out}" ]] && continue
			local status
			local bats
			status=$(basename "${out}" | cut -d '-' -f1)
			bats=$(basename "${out}" | cut -d '-' -f2- | sed 's/.out$//')
			echo " ${status} ${bats}"
		done

		echo -e "\nOUTPUTS:"
		for out in "${not_ok[@]}" "${ok[@]}"; do
			[[ -z "${out}" ]] && continue
			local bats
			bats=$(basename "${out}" | cut -d '-' -f2- | sed 's/.out$//')
			echo "::group::${bats}"
			cat "${out}"
			echo "::endgroup::"
		done
	done
}
