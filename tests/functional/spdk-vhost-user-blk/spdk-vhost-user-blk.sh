#!/bin/bash
#
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# End-to-end test of SPDK vhost-user-blk direct volumes (volume-type spdkvol).
#
# Every iteration runs the whole life cycle of a volume:
# - a new ext4 image is exposed through an SPDK vhost-blk controller and
#   registered with "kata-ctl direct-volume add";
# - a first container writes 32 MiB of deterministic data to the volume;
# - a second container, in a new VM, reads it back and checks its SHA-256;
# - the data is also read back from the image with debugfs;
# - once the containers are gone the device must have been removed: SPDK
#   refuses to delete a controller that a VM is still connected to.
# A directvol volume (an image file) is used as a control: it must not be
# attached as a vhost-user-blk device.
#
# SPDK needs huge page backed guest memory, so the VMs get it from a config.d
# drop-in next to the Kata configuration (KATA_CONFIG_PATH, which
# enabling_hypervisor sets), and the host must have the huge pages for them.
#
# Besides gha-run.sh, it can run on any host with Kata, containerd and a
# built SPDK, for example:
#   KATA_HYPERVISOR=clh-runtime-rs SPDK_DIR=/opt/spdk \
#   KATA_CONFIG_PATH=/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml \
#   ./spdk-vhost-user-blk.sh

[[ -n "${DEBUG:-}" ]] && set -x
set -o errexit
set -o nounset
set -o pipefail

this_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=/dev/null
source "${this_dir}/../../common.bash"

SPDK_DIR="${SPDK_DIR:-/opt/spdk}"
ITERATIONS="${ITERATIONS:-5}"
IMAGE="${IMAGE:-quay.io/prometheus/busybox:latest}"
KATA_CTL="${KATA_CTL:-kata-ctl}"
# Containerd socket, when not the default one
CONTAINERD_ADDRESS="${CONTAINERD_ADDRESS:-}"
RUNTIME="${RUNTIME:-io.containerd.kata-${KATA_HYPERVISOR:-clh-runtime-rs}.v2}"
# Extra "ctr run" arguments, e.g. "--runtime-config-path <file>"
CTR_RUN_ARGS="${CTR_RUN_ARGS:-}"
DATA_SIZE=$((32 * 1024 * 1024))
# Without an IOMMU, SPDK maps all of the guest memory when the guest activates
# the disk, and on a slow host that can outlast the 3 s the agent waits for it.
HOTPLUG_TIMEOUT_ANNOTATION="io.katacontainers.config.hypervisor.kernel_params=agent.hotplug_timeout=30"
KATA_CONFIG_PATH="${KATA_CONFIG_PATH:-}"
# Memory of the test VMs and of the SPDK target, in MiB, both from huge pages
GUEST_MEMORY_MB=2048
SPDK_MEMORY_MB=512

ctr=(ctr)
[[ -n "${CONTAINERD_ADDRESS}" ]] && ctr+=(--address "${CONTAINERD_ADDRESS}")

work_dir=""
spdk_pid=""
volume=""
controller=""
bdev=""
dropin=""
nr_hugepages=""

function rpc() {
	sudo "${SPDK_DIR}/scripts/rpc.py" -s "${work_dir}/spdk.sock" "$@"
}

function cleanup() {
	local rc=$?

	[[ -n "${volume}" ]] && sudo "${KATA_CTL}" direct-volume remove "${volume}" >/dev/null 2>&1
	if [[ -n "${spdk_pid}" ]]; then
		[[ "${rc}" -ne 0 ]] && sudo tail -n 50 "${work_dir}/spdk.log"
		sudo kill "${spdk_pid}" 2>/dev/null || true
		wait "${spdk_pid}" 2>/dev/null || true
	fi
	[[ -n "${work_dir}" ]] && sudo rm -rf "${work_dir}"
	[[ -n "${dropin}" ]] && sudo rm -f "${dropin}"
	[[ -n "${nr_hugepages}" ]] && echo "${nr_hugepages}" | sudo tee /proc/sys/vm/nr_hugepages >/dev/null
	return "${rc}"
}

# Huge pages for the SPDK target and one test VM at a time.
function reserve_hugepages() {
	local page_kb need free

	page_kb="$(awk '/^Hugepagesize/ {print $2}' /proc/meminfo)"
	need=$(((GUEST_MEMORY_MB + SPDK_MEMORY_MB) * 1024 / page_kb + 16))
	free="$(awk '/^HugePages_Free/ {print $2}' /proc/meminfo)"
	[[ "${free}" -ge "${need}" ]] && return
	nr_hugepages="$(cat /proc/sys/vm/nr_hugepages)"
	echo $((nr_hugepages + need - free)) | sudo tee /proc/sys/vm/nr_hugepages >/dev/null
	free="$(awk '/^HugePages_Free/ {print $2}' /proc/meminfo)"
	[[ "${free}" -ge "${need}" ]] || die "${need} free huge pages are needed, the host has ${free}"
}

# SPDK supports only huge page backed guest memory, and the annotation for it
# is not enabled by default.
function enable_guest_hugepages() {
	local hypervisor="${KATA_HYPERVISOR:-clh-runtime-rs}"

	[[ -n "${KATA_CONFIG_PATH}" ]] ||
		die "set KATA_CONFIG_PATH to the Kata configuration that ${RUNTIME} uses"
	hypervisor="${hypervisor%-runtime-rs}"
	dropin="$(dirname "${KATA_CONFIG_PATH}")/config.d/90-spdk-vhost-user-blk-test.toml"
	sudo mkdir -p "$(dirname "${dropin}")"
	printf '[hypervisor.%s]\nenable_hugepages = true\ndefault_memory = %d\n' \
		"${hypervisor}" "${GUEST_MEMORY_MB}" | sudo tee "${dropin}" >/dev/null
}

function start_spdk() {
	# The log goes to our own work directory, no privilege is needed for it.
	# shellcheck disable=SC2024
	sudo "${SPDK_DIR}/build/bin/vhost" -S "${work_dir}" -m 0x1 -s "${SPDK_MEMORY_MB}" \
		-r "${work_dir}/spdk.sock" >"${work_dir}/spdk.log" 2>&1 &
	spdk_pid=$!

	waitForProcess 30 1 "rpc spdk_get_version >/dev/null 2>&1" ||
		die "the SPDK vhost target did not start"
}

# count_logged <since> <message>: how often runtime-rs logged <message>
function count_logged() {
	sudo journalctl -t kata --since "$1" --no-pager -o cat | grep -c "$2" || true
}

# Number of VMs connected to the SPDK controller $1.
function controller_sessions() {
	rpc vhost_get_controllers -n "$1" | jq '.[0].sessions | length'
}

# run_workload <volume-type> <name> <command>
function run_workload() {
	local volume_type="$1"
	local name="$2"
	local cmd="$3"

	# shellcheck disable=SC2086
	sudo timeout 300 "${ctr[@]}" run --rm --runtime "${RUNTIME}" ${CTR_RUN_ARGS} \
		--annotation "${HOTPLUG_TIMEOUT_ANNOTATION}" \
		--mount "type=${volume_type},src=${volume},dst=/volume,options=rbind:rw" \
		--mount "type=bind,src=${work_dir}/input,dst=/input,options=rbind:ro" \
		"${IMAGE}" "${name}" sh -c "${cmd}"
}

# add_volume <volume-type> <device>
function add_volume() {
	volume="/run/kata-spdk-test/volume-$1-${RANDOM}"
	sudo mkdir -p "${volume}"
	sudo "${KATA_CTL}" direct-volume add "${volume}" \
		"{\"volume-type\": \"$1\", \"device\": \"$2\", \"fstype\": \"ext4\"}"
}

function remove_volume() {
	sudo "${KATA_CTL}" direct-volume remove "${volume}"
	sudo rmdir "${volume}"
	volume=""
}

function new_image() {
	local image="$1"

	sudo truncate -s 1G "${image}"
	sudo mkfs.ext4 -q -F "${image}"
}

function test_spdk_volume() {
	local iteration="$1"
	local image="${work_dir}/disk${iteration}.img"
	local since
	local out
	local sha

	info "spdkvol iteration ${iteration}/${ITERATIONS}"

	new_image "${image}"
	bdev="aio${iteration}"
	controller="vhost-blk${iteration}"
	rpc bdev_aio_create "${image}" "${bdev}" 512 >/dev/null
	rpc vhost_create_blk_controller "${controller}" "${bdev}" >/dev/null
	add_volume spdkvol "${work_dir}/${controller}"

	since="$(date '+%Y-%m-%d %H:%M:%S')"
	out="$(run_workload spdkvol "spdk-writer-${iteration}" \
		'cp /input/data /volume/data && sync && sha256sum /volume/data' 2>&1)" ||
		die "the writer failed: ${out}"
	[[ "${out}" == "${expected_sha}  /volume/data" ]] ||
		die "the writer saw '${out}', expected SHA-256 ${expected_sha}"

	out="$(run_workload spdkvol "spdk-reader-${iteration}" 'sha256sum /volume/data' 2>&1)" ||
		die "the reader failed: ${out}"
	[[ "${out}" == "${expected_sha}  /volume/data" ]] ||
		die "a new VM read '${out}', expected SHA-256 ${expected_sha}"

	[[ "$(count_logged "${since}" "vhost-user-blk device hot-plugged")" -eq 2 ]] ||
		die "spdkvol was not attached as a vhost-user-blk device twice"
	# Each removal is logged once the device has left the VM.
	[[ "$(count_logged "${since}" "vhost-user-blk device removed")" -eq 2 ]] ||
		die "the vhost-user-blk device was not removed after each workload"

	# SPDK ends the session shortly after the VM has closed the connection,
	# and refuses to delete a controller that still has one.
	waitForProcess 10 1 "[[ \$(controller_sessions ${controller}) -eq 0 ]]" ||
		die "a VM is still connected to ${controller}"
	rpc vhost_delete_controller "${controller}"
	rpc bdev_aio_delete "${bdev}"
	remove_volume

	sha="$(sudo debugfs -R "cat /data" "${image}" 2>/dev/null | sha256sum | cut -d' ' -f1)"
	[[ "${sha}" == "${expected_sha}" ]] ||
		die "the backing image holds SHA-256 '${sha}', expected ${expected_sha}"

	sudo rm -f "${image}"
	info "spdkvol iteration ${iteration}: SHA-256 ${expected_sha} written, read back from a new VM and from the image"
}

function test_directvol_is_not_vhost_user() {
	local image="${work_dir}/directvol.img"
	local since
	local out

	info "directvol control"

	new_image "${image}"
	add_volume directvol "${image}"

	since="$(date '+%Y-%m-%d %H:%M:%S')"
	out="$(run_workload directvol directvol-writer \
		'cp /input/data /volume/data && sync && sha256sum /volume/data' 2>&1)" ||
		die "the directvol writer failed: ${out}"
	[[ "${out}" == "${expected_sha}  /volume/data" ]] ||
		die "directvol: saw '${out}', expected SHA-256 ${expected_sha}"
	[[ "$(count_logged "${since}" "vhost-user-blk device hot-plugged")" -eq 0 ]] ||
		die "directvol was attached as a vhost-user-blk device"

	remove_volume
	sudo rm -f "${image}"
}

function main() {
	trap cleanup EXIT

	work_dir="$(mktemp -d /tmp/kata-spdk-vhost-user-blk.XXXXXX)"
	mkdir -p "${work_dir}/input"

	# Deterministic pseudo-random payload: AES-CTR keeps the length.
	head -c "${DATA_SIZE}" /dev/zero |
		openssl enc -aes-256-ctr -nosalt -K "$(printf '%064d' 0)" -iv "$(printf '%032d' 0)" \
			>"${work_dir}/input/data"
	expected_sha="$(sha256sum "${work_dir}/input/data" | cut -d' ' -f1)"

	sudo "${ctr[@]}" image pull "${IMAGE}" >/dev/null
	enable_guest_hugepages
	reserve_hugepages
	start_spdk

	local i
	for ((i = 1; i <= ITERATIONS; i++)); do
		test_spdk_volume "${i}"
	done

	test_directvol_is_not_vhost_user

	info "all ${ITERATIONS} spdkvol iterations passed"
}

main "$@"
