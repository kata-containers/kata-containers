#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")

SYSCONFIG_FILE="/etc/kata-containers/configuration.toml"

trap cleanup EXIT
cleanup() {
	sudo rm -rf "${SYSCONFIG_FILE}"
	# Don't fail the test if cleanup fails
	# VM will be destroyed anyway.
	${SCRIPT_DIR}/cleanup_env.sh || true
}

setup_configuration_file() {
	local image_type="$1"
	local machine_type="$2"
	local hypervisor="$3"
	local sandbox_cgroup_only="$4"

	local default_config_file="/opt/kata/share/defaults/kata-containers/configuration.toml"
	local qemu_config_file="/opt/kata/share/defaults/kata-containers/configuration-qemu.toml"
	local clh_config_file="/opt/kata/share/defaults/kata-containers/configuration-clh.toml"
	local image_file="/opt/kata/share/kata-containers/kata-containers.img"
	local initrd_file="/opt/kata/share/kata-containers/kata-containers-initrd.img"

	sudo mkdir -p $(dirname "${SYSCONFIG_FILE}")

	if [ "${hypervisor}" = "qemu" ]; then
		config_file="${qemu_config_file}"
	elif [ "${hypervisor}" = "cloud-hypervisor" ]; then
		config_file="${clh_config_file}"
	fi

	if [ -f "${config_file}" ]; then
		cp -a "${config_file}" "${SYSCONFIG_FILE}"
	elif [ -f "${default_config_file}" ]; then
		# Check if path contains the hypervisor name
		if ! grep "^path" "${default_config_file}" | grep -q "${hypervisor}"; then
			die "Configuration file for ${hypervisor} hypervisor not found"
		fi
		sudo cp -a "${default_config_file}" "${SYSCONFIG_FILE}"
	else
		die "Error: configuration file for ${hypervisor} doesn't exist"
	fi

	# machine type applies to configuration.toml and configuration-qemu.toml
	if [ -n "${machine_type}" ]; then
		if [ "${hypervisor}" = "qemu" ]; then
			sudo sed -i 's|^machine_type.*|machine_type = "'${machine_type}'"|g' "${SYSCONFIG_FILE}"
		else
			info "Variable machine_type only applies to qemu. It will be ignored"
		fi
	fi

	if [ -n "${sandbox_cgroup_only}" ]; then
		sudo sed -i 's|^sandbox_cgroup_only.*|sandbox_cgroup_only='${sandbox_cgroup_only}'|g' "${SYSCONFIG_FILE}"
	fi

	# Change to initrd or image depending on user input.
	# Non-default configs must be changed to specify either initrd or image, image is default.
	if [ "${image_type}" = "initrd" ]; then
		if $(grep -q "^image.*" "${SYSCONFIG_FILE}"); then
			if $(grep -q "^initrd.*" "${SYSCONFIG_FILE}"); then
				sudo sed -i '/^image.*/d' "${SYSCONFIG_FILE}"
			else
				sudo sed -i 's|^image.*|initrd = "'${initrd_file}'"|g' "${SYSCONFIG_FILE}"
			fi
		fi
	else
		if $(grep -q "^initrd.*" "${SYSCONFIG_FILE}"); then
			if $(grep -q "^image.*" "${SYSCONFIG_FILE}"); then
				sudo sed -i '/^initrd.*/d' "${SYSCONFIG_FILE}"
			else
				sudo sed -i 's|^initrd.*|image = "'${image_file}'"|g' "${SYSCONFIG_FILE}"
			fi
		fi
	fi

	# enable debug
	sudo sed -i -e 's/^#\(enable_debug\).*=.*$/\1 = true/g' \
		-e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug"/g' \
		"${SYSCONFIG_FILE}"

	# Cloud-hypervisor workaround
	# Issue: https://github.com/kata-containers/tests/issues/2963
	if [ "${hypervisor}" = "cloud-hypervisor" ]; then
		sudo sed -i -e 's|^default_memory.*|default_memory = 1024|g' \
		            -e 's|^virtio_fs_cache =.*|virtio_fs_cache = "none"|g' \
		            "${SYSCONFIG_FILE}"
	fi
}

run_test() {
	local image_type="${1:-}"
	# QEMU machine type
	local machine_type="${2:-}"
	local hypervisor="${3:-}"
	local sandbox_cgroup_only="${4:-}"

	info "Run test case: hypervisor=${hypervisor}" \
		"${machine_type:+machine=$machine_type}" \
		"image=${image_type} sandbox_cgroup_only=${sandbox_cgroup_only}"

	if [ -z "$image_type" ]; then
		die "need image type"
	elif [ -z "$hypervisor" ]; then
		die "need hypervisor"
	elif [ -z "$sandbox_cgroup_only" ]; then
		die "need sandbox cgroup only"
	fi

	setup_configuration_file "$image_type" "$machine_type" "$hypervisor" "$sandbox_cgroup_only"

	sudo -E kubectl create -f "${SCRIPT_DIR}/runtimeclass_workloads/vfio.yaml"

	pod_name=vfio
	sudo -E kubectl wait --for=condition=Ready pod "${pod_name}" || \
		{
			sudo -E kubectl describe pod "${pod_name}";
			die "Pod ${pod_name} failed to start";
		}

	# wait for the container to be ready
	waitForProcess 15 3 "sudo -E kubectl exec ${pod_name} -- ip a"

	# Expecting 2 network interaces -> 2 mac addresses
	mac_addrs=$(sudo -E kubectl exec "${pod_name}" -- ip a | grep "link/ether" | wc -l)
	if [ ${mac_addrs} -ne 2 ]; then
		die "Error: expecting 2 network interfaces, Got: $(kubectl exec "${pod_name}" -- ip a)"
	else
		info "Success: found 2 network interfaces"
	fi

	sudo -E kubectl delete -f "${SCRIPT_DIR}/runtimeclass_workloads/vfio.yaml"
}

main() {
	# Init k8s cluster
	${SCRIPT_DIR}/init.sh

	sudo modprobe vfio
	sudo modprobe vfio-pci

	# unbind device from driver
	# PCI address
	local addr="00:03.0"
	echo 0000:${addr} | sudo tee /sys/bus/pci/devices/0000:${addr}/driver/unbind

	# Create a new VFIO device
	# Ethernet controller: Red Hat, Inc. Virtio network device
	# vendor ID: 1af4
	# device ID: 1041
	local vendor_id="1af4"
	local device_id="1041"
	echo "${vendor_id} ${device_id}" | sudo tee /sys/bus/pci/drivers/vfio-pci/new_id

	# Install network (device) plugin
	sriov_plugin_url=$(get_version "plugins.sriov-network-device.url")
	sriov_plugin_version=$(get_version "plugins.sriov-network-device.version")
	git clone "${sriov_plugin_url}"
	pushd sriov-network-device-plugin
	git checkout "${sriov_plugin_version}"
	sed -i 's|resourceList.*|resourceList": [{"resourceName":"virtio_net","selectors":{"vendors":["'"${vendor_id}"'"],"devices":["'"${device_id}"'"],"drivers":["vfio-pci"],"pfNames":["eth1"]}},{|g' deployments/configMap.yaml
	sudo -E kubectl create -f deployments/configMap.yaml
	sudo -E kubectl create -f deployments/k8s-v1.16/sriovdp-daemonset.yaml
	sleep 5
	popd
	rm -rf sriov-network-device-plugin

	sriov_pod="$(sudo -E kubectl --namespace=kube-system get pods --output=name | grep sriov-device-plugin | cut -d/ -f2)"
	sudo -E kubectl --namespace=kube-system wait --for=condition=Ready pod "${sriov_pod}"

	# wait for the virtio_net resource
	for _ in $(seq 1 30); do
		v="$(sudo -E kubectl get node $(hostname | awk '{print tolower($0)}') -o json | jq '.status.allocatable["intel.com/virtio_net"]')"
		[ "${v}" == \"1\" ] && break
		sleep 5
	done

	# Skip clh/QEMU + initrd:
	# https://github.com/kata-containers/kata-containers/issues/900
	# run_test initrd "" cloud-hypervisor false
	# run_test initrd "" cloud-hypervisor false
	# run_test initrd "q35" qemu false
	# run_test initrd "q35" qemu true
	run_test image "" cloud-hypervisor false
	run_test image "" cloud-hypervisor true
	run_test image "q35" qemu false
	run_test image "q35" qemu true
}

main $@
