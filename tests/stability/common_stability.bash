#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

STABILITY_DIR=$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")
# shellcheck disable=SC1091
source "${STABILITY_DIR}/../common.bash"
source /etc/os-release || source /usr/lib/os-release

# Set variables to reasonable defaults if unset or empty
CTR_EXE="${CTR_EXE:-ctr}"
DOCKER_EXE="${DOCKER_EXE:-docker}"
CTR_RUNTIME="${CTR_RUNTIME:-io.containerd.kata.v2}"
RUNTIME="${RUNTIME:-containerd-shim-kata-v2}"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"

declare -A registries
registries[ubuntu]=\
"docker.io/library
public.ecr.aws/lts
mirror.gcr.io/library
quay.io/libpod"

function check_cmds()
{
	local cmd req_cmds=( "$@" )
	for cmd in "${req_cmds[@]}"; do
		if ! command -v "${cmd}" > /dev/null 2>&1; then
			die "command ${cmd} not available"
		fi
		info "command: ${cmd}: yes"
	done
}

function generate_build_dockerfile()
{
	local dockerfile="$1"
	local image="$2"
	local map_key="$3"
	local text_to_replace="$4"
	local regs
	read -r -a regs <<< "${registries["${map_key}"]}"

	for r in "${regs[@]}"; do
		sed 's|'"${text_to_replace}"'|'"${r}"'|g' \
			"${dockerfile}.in" > "${dockerfile}"
		if sudo -E "${DOCKER_EXE}" build \
			--build-arg http_proxy="${http_proxy}" --build-arg https_proxy="${https_proxy}" \
			--build-arg HTTP_PROXY="${http_proxy}" --build-arg HTTPS_PROXY="${https_proxy}" \
			--label "${image}" --tag "${image}" -f "${dockerfile}" "${dockerfile_dir}"; then
			return 0
		fi
	done
	return 1
}

function build_dockerfile_image()
{
	local image="$1"
	local dockerfile_path="$2"
	local dockerfile_dir="${2%/*}"

	if [[ -f "${dockerfile_path}" ]]; then
		info "docker building ${image}"
		if ! sudo -E "${DOCKER_EXE}" build \
			--build-arg http_proxy="${http_proxy}" --build-arg https_proxy="${https_proxy}" \
			--build-arg HTTP_PROXY="${http_proxy}" --build-arg HTTPS_PROXY="${https_proxy}" \
			--label "${image}" --tag "${image}" -f "${dockerfile_path}" "${dockerfile_dir}"; then
			die "Failed to docker build image ${image}"
		fi
		return 0
	fi

	generate_build_dockerfile "${dockerfile_path}" "${image}" "ubuntu" "@UBUNTU_REGISTRY@" \
		|| die "Failed to docker build image ${image}"
}

function check_ctr_images()
{
	local ctr_image="$1"
	local dockerfile_path="$2"
	local docker_image
	docker_image="$(echo "${ctr_image}" | cut -d/ -f3 | cut -d: -f1)"

	if [[ -z "${ctr_image}" ]] || [[ -z "${dockerfile_path}" ]]; then
		die "Missing image or dockerfile path variable"
	fi

	sudo "${CTR_EXE}" i rm "${ctr_image}"
	build_dockerfile_image "${docker_image}" "${dockerfile_path}"
	sudo "${DOCKER_EXE}" save -o "${docker_image}.tar" "${docker_image}"
	sudo "${CTR_EXE}" i import "${docker_image}.tar"
	rm -rf "${docker_image}".tar
}

function test_banner()
{
	info -e "\n===== starting test [$1] ====="
}

# TEST_NAME is expected to be set by the caller
function init_env()
{
	# shellcheck disable=SC2154
	test_banner "${TEST_NAME}"

	cmd=("docker" "ctr")

	check_cmds "${cmd[@]}"

	clean_env_ctr

	docker_masked="$(systemctl list-unit-files --state=masked | grep -c docker)" || true
	[[ "${docker_masked}" -eq 0 ]] && sudo systemctl restart docker

	kill_processes_before_start
	check_processes
	info "init environment complete"
}

function kill_processes_before_start()
{
	docker_masked="$(systemctl list-unit-files --state=masked | grep -c "${DOCKER_EXE}")" || true

	if [[ "${docker_masked}" -eq 0 ]]; then
		DOCKER_PROCS=$(sudo "${DOCKER_EXE}" ps -q)
		[[ -n "${DOCKER_PROCS}" ]] && clean_env
	fi

	CTR_PROCS=$(sudo "${CTR_EXE}" t list -q)
	[[ -n "${CTR_PROCS}" ]] && clean_env_ctr

	restart_containerd_service

	kill_kata_components
}

function kill_kata_components() {
	local ATTEMPTS=2
	local TIMEOUT="300s"
	local PID_NAMES=( "containerd-shim-kata-v2" "qemu-system-x86_64" "qemu-system-x86_64-tdx-experimental" "cloud-hypervisor" )

	sudo systemctl stop containerd
	for (( i=1; i<=ATTEMPTS; i++ )); do
		for PID_NAME in "${PID_NAMES[@]}"; do
			if [[ -n "$(pidof "${PID_NAME}")" ]]; then
				sudo killall -w -s SIGKILL "${PID_NAME}" >/dev/null 2>&1 || true
			fi
		done
		sleep 1
	done
	sudo timeout -s SIGKILL "${TIMEOUT}" systemctl start containerd
}

function random_name()
{
	mktemp -u kata-XXXXXX
}

function show_system_ctr_state()
{
	info "Showing system state:"
	info " --Check containers--"
	sudo "${CTR_EXE}" c list
	info " --Check tasks--"
	sudo "${CTR_EXE}" task list

	local processes="containerd-shim-kata-v2"

	for p in ${processes}; do
		info " --pgrep ${p}--"
		pgrep -a "${p}"
	done
}

function check_containers_are_up() {
	local NUM_CONTAINERS="$1"
	[[ -z "${NUM_CONTAINERS}" ]] && die "Number of containers is missing"

	local TIMEOUT=60
	local containers_launched=0
	for i in $(seq "${TIMEOUT}") ; do
		info "Verify that the containers are running"
		containers_launched="$(sudo "${CTR_EXE}" t list | grep -c "RUNNING")"
		[[ "${containers_launched}" -eq "${NUM_CONTAINERS}" ]] && break
		sleep 1
		[[ "${i}" == "${TIMEOUT}" ]] && return 1
	done
}

function check_containers_are_running() {
	local NUM_CONTAINERS="$1"
	[[ -z "${NUM_CONTAINERS}" ]] && die "Number of containers is missing"

	local timeout_launch="10"
	check_containers_are_up "${NUM_CONTAINERS}" & pid=$!
	(sleep "${timeout_launch}" && kill -HUP "${pid}") 2>/dev/null & pid_tout=$!

	if wait "${pid}" 2>/dev/null; then
		pkill -HUP -P "${pid_tout}"
		wait "${pid_tout}"
	else
		warn "Time out exceeded"
		return 1
	fi
}
