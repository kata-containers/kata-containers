#!/bin/bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_path=$(dirname "$0")
source "${script_path}/../../common.bash"

addr=
tmp_data_dir="$(mktemp -d)"
rootfs_tar="${tmp_data_dir}/rootfs.tar"
trap cleanup EXIT

# kata-runtime options
SANDBOX_CGROUP_ONLY=""
HYPERVISOR=
MACHINE_TYPE=
IMAGE_TYPE=

cleanup() {
	clean_env_ctr
	sudo rm -rf "${tmp_data_dir}"

	[ -n "${host_pci}" ] && sudo driverctl unset-override "${host_pci}"
}

host_pci_addr() {
	lspci -D | grep "Ethernet controller" | grep "Virtio.*network device" | tail -1 | cut -d' ' -f1
}

get_vfio_path() {
	local addr="$1"
	echo "/dev/vfio/$(basename $(realpath /sys/bus/pci/drivers/vfio-pci/${host_pci}/iommu_group))"
}

pull_rootfs() {
	# pull and export busybox image in tar file
	local image="quay.io/prometheus/busybox:latest"
	sudo -E ctr i pull ${image}
	sudo -E ctr i export "${rootfs_tar}" "${image}"
	sudo chown ${USER}:${USER} "${rootfs_tar}"
	sync
}

create_bundle() {
	local bundle_dir="$1"
	mkdir -p "${bundle_dir}"

	# extract busybox rootfs
	local rootfs_dir="${bundle_dir}/rootfs"
	mkdir -p "${rootfs_dir}"
	local layers_dir="$(mktemp -d)"
	tar -C "${layers_dir}" -pxf "${rootfs_tar}"
	for ((i=0;i<$(cat ${layers_dir}/manifest.json | jq -r ".[].Layers | length");i++)); do
		tar -C ${rootfs_dir} -xf ${layers_dir}/$(cat ${layers_dir}/manifest.json | jq -r ".[].Layers[${i}]")
	done
	sync

	# Copy config.json
	cp -a "${script_path}/config.json" "${bundle_dir}/config.json"
}

run_container() {
	local container_id="$1"
	local bundle_dir="$2"

	sudo -E ctr run -d --runtime io.containerd.kata.v2 --config "${bundle_dir}/config.json" "${container_id}"
}


get_ctr_cmd_output() {
	local container_id="$1"
	shift
	timeout 30s sudo -E ctr t exec --exec-id 2 "${container_id}" "${@}"
}

check_guest_kernel() {
	local container_id="$1"
	# For vfio_mode=guest-kernel, the device should be bound to
	# the guest kernel's native driver.  To check this has worked,
	# we look for an ethernet device named 'eth*'
	get_ctr_cmd_output "${container_id}" ip a | grep "eth" || die "Missing VFIO network interface"
}

check_vfio() {
	local cid="$1"
	# For vfio_mode=vfio, the device should be bound to the guest
	# vfio-pci driver.

	# Check the control device is visible
	get_ctr_cmd_output "${cid}" ls /dev/vfio/vfio || die "Couldn't find VFIO control device in container"

	# The device should *not* cause an ethernet interface to appear
	! get_ctr_cmd_output "${cid}" ip a | grep "eth" || die "Unexpected network interface"

	# There should be exactly one VFIO group device (there might
	# be multiple IOMMU groups in the VM, but only one device
	# should be bound to the VFIO driver, so there should still
	# only be one VFIO device
	group="$(get_ctr_cmd_output "${cid}" ls /dev/vfio | grep -v vfio)"
	if [ $(echo "${group}" | wc -w) != "1" ] ; then
	    die "Expected exactly one VFIO group got: ${group}"
	fi

	# There should be two devices in the IOMMU group: the ethernet
	# device we care about, plus the PCIe to PCI bridge device
	devs="$(get_ctr_cmd_output "${cid}" ls /sys/kernel/iommu_groups/"${group}"/devices)"
	num_devices=$(echo "${devs}" | wc -w)
	if [ "${HYPERVISOR}" = "qemu" ] && [ "${num_devices}" != "2" ] ; then
	    die "Expected exactly two devices got: ${devs}"
	fi
	if [ "${HYPERVISOR}" = "clh" ] && [ "${num_devices}" != "1" ] ; then
	    die "Expected exactly one device got: ${devs}"
	fi

	# The bridge device will always sort first, because it is on
	# bus zero, whereas the NIC will be on a non-zero bus
	guest_pci=$(echo "${devs}" | tail -1)

	# This is a roundabout way of getting the environment
	# variable, but to use the more obvious "echo $PCIDEVICE_..."
	# we would have to escape the '$' enough to not be expanded
	# before it's injected into the container, but not so much
	# that it *is* expanded by the shell within the container.
	# Doing that with another shell function in between is very
	# fragile, so do it this way instead.
	guest_env="$(get_ctr_cmd_output "${cid}" env | grep ^PCIDEVICE_VIRTIO_NET | sed s/^[^=]*=//)"
	if [ "${guest_env}" != "${guest_pci}" ]; then
	    die "PCIDEVICE variable was \"${guest_env}\" instead of \"${guest_pci}\""
	fi
}

get_dmesg() {
	local container_id="$1"
	get_ctr_cmd_output "${container_id}" dmesg
}

# Show help about this script
help(){
cat << EOF
Usage: $0 [-h] [options]
    Description:
        This script runs a kata container and passthrough a vfio device
    Options:
        -h,          Help
        -i <string>, Specify initrd or image
        -m <string>, Specify kata-runtime machine type for qemu hypervisor
        -p <string>, Specify kata-runtime hypervisor
        -s <value>,  Set sandbox_cgroup_only in the configuration file
EOF
}

setup_configuration_file() {
	local qemu_config_file="configuration-qemu.toml"
	local clh_config_file="configuration-clh.toml"
	local image_file="/opt/kata/share/kata-containers/kata-containers.img"
	local initrd_file="/opt/kata/share/kata-containers/kata-containers-initrd.img"
	local kata_config_file=""

	for file in $(kata-runtime --kata-show-default-config-paths); do
		if [ ! -f "${file}" ]; then
			continue
		fi

		kata_config_file="${file}"
		config_dir=$(dirname ${file})
		config_filename=""

		if [ "$HYPERVISOR" = "qemu" ]; then
			config_filename="${qemu_config_file}"
		elif [ "$HYPERVISOR" = "clh" ]; then
			config_filename="${clh_config_file}"
		fi

		config_file="${config_dir}/${config_filename}"
		if [ -f "${config_file}" ]; then
			rm -f "${kata_config_file}"
			cp -a $(realpath "${config_file}") "${kata_config_file}"
			break
		fi
	done

	# machine type applies to configuration.toml and configuration-qemu.toml
	if [ -n "$MACHINE_TYPE" ]; then
		if [ "$HYPERVISOR" = "qemu" ]; then
			sed -i 's|^machine_type.*|machine_type = "'${MACHINE_TYPE}'"|g' "${kata_config_file}"
		else
			warn "Variable machine_type only applies to qemu. It will be ignored"
		fi
	fi

	# Make sure we have set hot_plug_vfio to a reasonable value
	if [ "$HYPERVISOR" = "qemu" ]; then
		sed -i -e 's|^#*.*hot_plug_vfio.*|hot_plug_vfio = "bridge-port"|' "${kata_config_file}"
	elif [ "$HYPERVISOR" = "clh" ]; then
		sed -i -e 's|^#*.*hot_plug_vfio.*|hot_plug_vfio = "root-port"|' "${kata_config_file}"
	fi

	if [ -n "${SANDBOX_CGROUP_ONLY}" ]; then
	   sed -i 's|^sandbox_cgroup_only.*|sandbox_cgroup_only='${SANDBOX_CGROUP_ONLY}'|g' "${kata_config_file}"
	fi

	# Change to initrd or image depending on user input.
	# Non-default configs must be changed to specify either initrd or image, image is default.
	if [ "$IMAGE_TYPE" = "initrd" ]; then
		if $(grep -q "^image.*" ${kata_config_file}); then
			if $(grep -q "^initrd.*" ${kata_config_file}); then
				sed -i '/^image.*/d' "${kata_config_file}"
			else
				sed -i 's|^image.*|initrd = "'${initrd_file}'"|g' "${kata_config_file}"
			fi
		fi
	else
		if $(grep -q "^initrd.*" ${kata_config_file}); then
			if $(grep -q "^image.*" ${kata_config_file}); then
				sed -i '/^initrd.*/d' "${kata_config_file}"
			else
				sed -i 's|^initrd.*|image = "'${image_file}'"|g' "${kata_config_file}"
			fi
		fi
	fi

	# enable debug
	sed -i -e 's/^#\(enable_debug\).*=.*$/\1 = true/g' \
	       -e 's/^#\(debug_console_enabled\).*=.*$/\1 = true/g' \
	       -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 mitigations=off agent.log=debug"/g' \
	       "${kata_config_file}"

	# enable VFIO relevant hypervisor annotations
	sed -i -e 's/^\(enable_annotations\).*=.*$/\1 = ["enable_iommu"]/' \
		"${kata_config_file}"
}

run_test_container() {
	local container_id="$1"
	local bundle_dir="$2"
	local config_json_in="$3"
	local host_pci="$4"

	# generate final config.json
	sed -e '/^#.*/d' \
	    -e 's|@VFIO_PATH@|'"${vfio_device}"'|g' \
	    -e 's|@VFIO_MAJOR@|'"${vfio_major}"'|g' \
	    -e 's|@VFIO_MINOR@|'"${vfio_minor}"'|g' \
	    -e 's|@VFIO_CTL_MAJOR@|'"${vfio_ctl_major}"'|g' \
	    -e 's|@VFIO_CTL_MINOR@|'"${vfio_ctl_minor}"'|g' \
	    -e 's|@ROOTFS@|'"${bundle_dir}/rootfs"'|g' \
	    -e 's|@HOST_PCI@|'"${host_pci}"'|g' \
	    "${config_json_in}" > "${script_path}/config.json"

	create_bundle "${bundle_dir}"

	# run container
	run_container "${container_id}" "${bundle_dir}"

	# output VM dmesg
	get_dmesg "${container_id}"
}

main() {
	local OPTIND
	while getopts "hi:m:p:s:" opt;do
		case ${opt} in
		h)
		    help
		    exit 0;
		    ;;
		i)
		    IMAGE_TYPE="${OPTARG}"
		    ;;
		m)
		    MACHINE_TYPE="${OPTARG}"
		    ;;
		p)
		    HYPERVISOR="${OPTARG}"
		    ;;
		s)
		    SANDBOX_CGROUP_ONLY="${OPTARG}"
		    ;;
		?)
		    # parse failure
		    help
		    die "Failed to parse arguments"
		    ;;
		esac
	done
	shift $((OPTIND-1))

	#
	# Get the device ready on the host
	#
	setup_configuration_file

	restart_containerd_service
	sudo modprobe vfio
	sudo modprobe vfio-pci

	host_pci=$(host_pci_addr)
	[ -n "${host_pci}" ] || die "virtio ethernet controller PCI address not found"

	cat /proc/cmdline | grep -q "intel_iommu=on" || \
		die "intel_iommu=on not found in kernel cmdline"

	sudo driverctl set-override "${host_pci}" vfio-pci

	vfio_device="$(get_vfio_path "${host_pci}")"
	[ -n "${vfio_device}" ] || die "vfio device not found"
	vfio_major="$(printf '%d' $(stat -c '0x%t' ${vfio_device}))"
	vfio_minor="$(printf '%d' $(stat -c '0x%T' ${vfio_device}))"

	[ -n "/dev/vfio/vfio" ] || die "vfio control device not found"
	vfio_ctl_major="$(printf '%d' $(stat -c '0x%t' /dev/vfio/vfio))"
	vfio_ctl_minor="$(printf '%d' $(stat -c '0x%T' /dev/vfio/vfio))"

	# Get the rootfs we'll use for all tests
	pull_rootfs

	#
	# Run the tests
	#

	# test for guest-kernel mode
	guest_kernel_cid="vfio-guest-kernel-${RANDOM}"
	run_test_container "${guest_kernel_cid}" \
			   "${tmp_data_dir}/vfio-guest-kernel" \
			   "${script_path}/guest-kernel.json.in" \
			   "${host_pci}"
	check_guest_kernel "${guest_kernel_cid}"

	# Remove the container so we can re-use the device for the next test
	clean_env_ctr

	# test for vfio mode
	vfio_cid="vfio-vfio-${RANDOM}"
	run_test_container "${vfio_cid}" \
			   "${tmp_data_dir}/vfio-vfio" \
			   "${script_path}/vfio.json.in" \
			   "${host_pci}"
	check_vfio "${vfio_cid}"
}

main $@
