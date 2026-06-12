#!/bin/bash
#
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This test file will test kata-monitor for basic functionality (retrieve kata sandboxes)
# It will assume an environment where:
# - a CRI container manager (container engine) will be up and running
# - crictl is installed and configured
# - the kata-monitor binary is available on the host
#

set -o errexit
set -o nounset
set -o pipefail

# shellcheck source=/dev/null
source "/etc/os-release" || source "/usr/lib/os-release"

[[ -n "${BASH_VERSION:-}" ]] && set -o errtrace
[[ -n "${DEBUG:-}" ]] && set -o xtrace

readonly MONITOR_HTTP_ENDPOINT="127.0.0.1:8090"
# we should collect few hundred metrics, let's put a reasonable minimum
readonly MONITOR_MIN_METRICS_NUM=200
readonly TIMEOUT="20s"
CONTAINER_ENGINE=${CONTAINER_ENGINE:-"containerd"}
CRICTL_RUNTIME=${CRICTL_RUNTIME:-"kata"}
# When KATA_MONITOR_IMAGE is set, kata-monitor runs inside that container
# image instead of as an on-disk binary. This is what CI does — it
# validates the actual image we ship. Manual runs without the env var
# keep the previous behaviour of executing ${KATA_MONITOR_BIN}.
KATA_MONITOR_IMAGE="${KATA_MONITOR_IMAGE:-}"
KATA_MONITOR_BIN="${KATA_MONITOR_BIN:-$(command -v kata-monitor || true)}"
readonly KATA_MONITOR_CONTAINER_NAME="kata-monitor-test"
KATA_MONITOR_PID=""
TMPATH=$(mktemp -d -t kata-monitor-test-XXXXXXXXX)
METRICS_FILE="${TMPATH}/metrics.txt"
MONITOR_LOG_FILE="${TMPATH}/kata-monitor.log"
CACHE_UPD_TIMEOUT_SEC=${CACHE_UPD_TIMEOUT_SEC:-20}
POD_ID=""
CID=""
RUNC_POD_ID=""
RUNC_CID=""
CURRENT_TASK=""

FALSE=1
TRUE=0

trap error_with_msg ERR

title() {
	local step="$1"
	echo -e "\n* STEP: ${step}"
}

echo_ok() {
	local msg="$1"

	echo "OK: ${msg}"
}

# quiet crictrl
qcrictl() {
	sudo crictl "$@" > /dev/null
}

# this is just an hash of current date (+ nanoseconds)
gen_unique_id() {
	date +%T:%N | md5sum | cut -d ' ' -f 1
}

error_with_msg() {
	local msg=${1:-"cannot ${CURRENT_TASK}"}

	trap - ERR
	echo -e "\nERROR: ${msg}"
	if [[ -f "${MONITOR_LOG_FILE}" ]]; then
		echo -e "\nkata-monitor logs:\n----------------"
		cat "${MONITOR_LOG_FILE}"
	fi
	echo -e "\nkata-monitor testing: FAILED!"
	cleanup
	exit 1
}

cleanup() {
	stop_workload
	stop_workload "${RUNC_CID}" "${RUNC_POD_ID}"

	stop_kata_monitor

	rm -rf "${TMPATH}"
}

start_kata_monitor() {
	local args="$1"

	if [[ -n "${KATA_MONITOR_IMAGE}" ]]; then
		# `--network host` keeps the default 127.0.0.1:8090 bind
		# reachable from the host-side test code without having to
		# publish a port. Mount /run/containerd so the monitor can
		# reach containerd's CRI socket, plus the kata sandbox base
		# path for the per-sandbox shim-monitor sockets.

		# Ensure /run/vc/sbs/ exists on the host so the readonly mount
		# does not fail before the first kata sandbox is created.
		sudo mkdir -p /run/vc/sbs

		# shellcheck disable=SC2086
		sudo docker run --rm -d \
			--name "${KATA_MONITOR_CONTAINER_NAME}" \
			--network host \
			-v /run/containerd:/run/containerd:ro \
			-v /run/vc/sbs:/run/vc/sbs:ro \
			"${KATA_MONITOR_IMAGE}" \
			${args} --log-level trace > /dev/null
		# Stream container logs into the same file the binary path
		# writes to, so error_with_msg's dump works identically in
		# both modes. The redirect target lives under our own
		# ${TMPATH}, so SC2024 (sudo doesn't affect redirects) is a
		# false positive here.
		# shellcheck disable=SC2024
		sudo docker logs -f "${KATA_MONITOR_CONTAINER_NAME}" \
			> "${MONITOR_LOG_FILE}" 2>&1 &
		return
	fi

	[[ ! -x "${KATA_MONITOR_BIN}" ]] && error_with_msg "kata-monitor binary not found"
	# shellcheck disable=SC2024,SC2086
	sudo "${KATA_MONITOR_BIN}" ${args} --log-level trace > "${MONITOR_LOG_FILE}" 2>&1 &
	KATA_MONITOR_PID="$!"
}

stop_kata_monitor() {
	if [[ -n "${KATA_MONITOR_IMAGE}" ]]; then
		sudo docker stop "${KATA_MONITOR_CONTAINER_NAME}" > /dev/null 2>&1 || true
		return
	fi

	[[ -n "${KATA_MONITOR_PID}" ]] \
		&& [[ -d "/proc/${KATA_MONITOR_PID}" ]] \
		&& kill -9 "${KATA_MONITOR_PID}"
}

create_sandbox_json() {
	local uid_name_suffix
	uid_name_suffix="$(gen_unique_id)"
	local sbfile="${TMPATH}/sandbox-${uid_name_suffix}.json"

	cat <<EOF >"${sbfile}"
{
	"metadata": {
		"name": "nginx-${uid_name_suffix}",
		"namespace": "default",
		"uid": "nginx-container-uid",
		"attempt": 1
	},
	"logDirectory": "/tmp",
	"linux": {
	}
}
EOF
	echo "${sbfile}"
}

create_container_json() {
	local uid_name_suffix
	uid_name_suffix="$(gen_unique_id)"
	local cntfile="${TMPATH}/container-${uid_name_suffix}.json"

	cat <<EOF >"${cntfile}"
{
	"metadata": {
		"name": "busybox",
		"namespace": "default",
		"uid": "busybox-container-uid"
	},
	"image":{
		"image": "busybox"
	},
	"command": [
		"top"
	],
	"log_path":"busybox.log",
	"linux": {
	}
}
EOF
	echo "${cntfile}"
}

start_workload() {
	local runtime=${1:-}
	local args=""
	local sbfile=""
	local cntfile=""

	[[ -n "${runtime}" ]] && args="-r ${runtime}"

	sbfile="$(create_sandbox_json)"
	cntfile="$(create_container_json)"

	# shellcheck disable=SC2086
	POD_ID=$(sudo crictl --timeout="${TIMEOUT}" runp ${args} "${sbfile}")
	CID=$(sudo crictl --timeout="${TIMEOUT}" create "${POD_ID}" "${cntfile}" "${sbfile}")
	qcrictl --timeout="${TIMEOUT}" start "${CID}"
}

stop_workload() {
	local cid="${1:-${CID}}"
	local pod_id="${2:-${POD_ID}}"
	local check

	[[ -z "${pod_id}" ]] && return
	check=$(sudo crictl --timeout="${TIMEOUT}" pods -q -id "${pod_id}")
	[[ -z "${check}" ]] && return

	qcrictl --timeout="${TIMEOUT}" stop "${cid}"
	qcrictl --timeout="${TIMEOUT}" rm "${cid}"

	qcrictl --timeout="${TIMEOUT}" stopp "${pod_id}"
	qcrictl --timeout="${TIMEOUT}" rmp "${pod_id}"
}

is_sandbox_there() {
	local podid=${1}
	local sbs s

	sbs=$(sudo curl -s "${MONITOR_HTTP_ENDPOINT}/sandboxes")
	if [[ -n "${sbs}" ]]; then
		for s in ${sbs}; do
			if [[ "${s}" = "${podid}" ]]; then
				return "${TRUE}"
			fi
		done
	fi
	return "${FALSE}"
}

is_sandbox_there_iterate() {
	local podid=${1}

	# shellcheck disable=SC2034
	for i in $(seq 1 "${CACHE_UPD_TIMEOUT_SEC}"); do
		is_sandbox_there "${podid}" && return "${TRUE}"
		echo -n "."
		sleep 1
		continue
	done

	return "${FALSE}"
}

is_sandbox_missing_iterate() {
	local podid=${1}

	# shellcheck disable=SC2034
	for i in $(seq 1 "${CACHE_UPD_TIMEOUT_SEC}"); do
		is_sandbox_there "${podid}" || return "${TRUE}"
		echo -n "."
		sleep 1
		continue
	done

	return "${FALSE}"
}

main() {
	local args=""

	###########################
	title "pre-checks"

	CURRENT_TASK="connect to the container engine"
	qcrictl --timeout="${TIMEOUT}" pods
	echo_ok "${CURRENT_TASK}"

	###########################
	title "pull the image to be used"
	sudo crictl --timeout="${TIMEOUT}" pull busybox

	###########################
	title "create workloads"

	CURRENT_TASK="start workload (runc)"
	start_workload runc
	RUNC_POD_ID="${POD_ID}"
	RUNC_CID="${CID}"
	echo_ok "${CURRENT_TASK} - POD ID:${POD_ID}, CID:${CID}"

	CURRENT_TASK="start workload (${CRICTL_RUNTIME})"
	start_workload "${CRICTL_RUNTIME}"
	echo_ok "${CURRENT_TASK} - POD ID:${POD_ID}, CID:${CID}"

	###########################
	title "start kata-monitor"

	CURRENT_TASK="start kata-monitor"
	start_kata_monitor "${args}"
	if [[ -n "${KATA_MONITOR_IMAGE}" ]]; then
		echo_ok "${CURRENT_TASK} (image ${KATA_MONITOR_IMAGE})"
	else
		echo_ok "${CURRENT_TASK} (pid ${KATA_MONITOR_PID})"
	fi

	###########################
	title "kata-monitor cache update checks"

	CURRENT_TASK="retrieve ${POD_ID} in kata-monitor cache"
	is_sandbox_there_iterate "${POD_ID}" || error_with_msg
	echo_ok "${CURRENT_TASK}"

	CURRENT_TASK="look for runc pod ${RUNC_POD_ID} in kata-monitor cache"
	is_sandbox_there_iterate "${RUNC_POD_ID}" && error_with_msg "cache: got runc pod ${RUNC_POD_ID}"
	echo_ok "runc pod ${RUNC_POD_ID} skipped from kata-monitor cache"

	###########################
	title "kata-monitor metrics retrieval"

	CURRENT_TASK="retrieve metrics from kata-monitor"
	curl -s "${MONITOR_HTTP_ENDPOINT}/metrics" > "${METRICS_FILE}"
	echo_ok "${CURRENT_TASK}"

	CURRENT_TASK="retrieve metrics for pod ${POD_ID}"
	METRICS_COUNT=$(grep -c "${POD_ID}" "${METRICS_FILE}" || true)
	[[ "${METRICS_COUNT}" -lt "${MONITOR_MIN_METRICS_NUM}" ]] \
		&& error_with_msg "got too few metrics (#${METRICS_COUNT})"
	echo_ok "${CURRENT_TASK} - found #${METRICS_COUNT} metrics"

	###########################
	title "remove kata workload"

	CURRENT_TASK="stop workload (${CRICTL_RUNTIME})"
	stop_workload
	echo_ok "${CURRENT_TASK}"

	###########################
	title "kata-monitor cache update checks (removal)"

	CURRENT_TASK="verify removal of ${POD_ID} from kata-monitor cache"
	is_sandbox_missing_iterate "${POD_ID}" || error_with_msg "pod ${POD_ID} was not removed"
	echo_ok "${CURRENT_TASK}"

	###########################
	CURRENT_TASK="cleanup"
	cleanup

	echo -e "\nkata-monitor testing: PASSED!\n"
}

main "$@"
