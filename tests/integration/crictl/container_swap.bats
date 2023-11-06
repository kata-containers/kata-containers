#!/usr/bin/env bats
#
# Copyright (c) 2017-2023 Intel Corporation
# Copyright (c) 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../cri-containerd/lib.sh"

function getContainerSwapInfo() {
	export swap_size=$(($(sudo crictl exec $cid cat /proc/meminfo | grep "SwapTotal:" | awk '{print $2}')*1024))
	# NOTE: these below two checks only works on cgroup v1
	export swappiness=$(sudo crictl exec $cid cat /sys/fs/cgroup/memory/memory.swappiness)
	export swap_in_bytes=$(sudo crictl exec $cid cat /sys/fs/cgroup/memory/memory.memsw.limit_in_bytes)
}

setup_file() {
	if [[ "${KATA_HYPERVISOR}" != "qemu" ]] || [[ "${ARCH}" != "x86_64" ]]; then
		skip "Tests supported for qemu hypervisor on x86_64 only"
	fi

	create_containerd_config "kata-${KATA_HYPERVISOR}" 1
	restart_containerd_service

	[ -f "$kata_config" ] && sudo cp "$kata_config" "$kata_config_backup" || \
		sudo cp "$default_kata_config" "$kata_config"

	sudo sed -i -e 's/^#enable_guest_swap.*$/enable_guest_swap = true/g' "${kata_config}"

	export container_yaml=${REPORT_DIR}/container.yaml
	export image="busybox:latest"
}

teardown_file() {
	create_containerd_config "kata-${KATA_HYPERVISOR}"
	restart_containerd_service

	[ -f "$kata_config_backup" ] && sudo mv "$kata_config_backup" "$kata_config" || \
		sudo rm "$kata_config"
}

setup() {
	# TestContainerSwap is currently failing with GHA.
	# Let's re-enable it as soon as we get it to work.
	# Reference: https://github.com/kata-containers/kata-containers/issues/7410
	skip "Currently failing with GHA (issue #7410)"
}

teardown() {
	skip "Currently failing with GHA (issue #7410)"

	testContainerStop
}

@test "Test with enabled guest swap and container without swap device" {
	# Test without swap device
	testContainerStart
	getContainerSwapInfo

	# Current default swappiness is 60
	if [ $swappiness -ne 60 ]; then
		testContainerStop
		echo "The VM swappiness $swappiness without swap device is not right"
		false
	fi
	if [ $swap_in_bytes -lt 1125899906842624 ]; then
		testContainerStop
		echo "The VM swap_in_bytes $swap_in_bytes without swap device is not right"
		false
	fi
	if [ $swap_size -ne 0 ]; then
		testContainerStop
		echo "The VM swap size $swap_size without swap device is not right"
		false
	fi
}

@test "Test with enabled guest swap and container with swap_in_bytes and memory_limit_in_bytes" {
	# Test with swap device
	cat << EOF > "${container_yaml}"
metadata:
  name: busybox-swap
  namespace: default
  uid: busybox-swap-uid
annotations:
  io.katacontainers.container.resource.swappiness: "100"
  io.katacontainers.container.resource.swap_in_bytes: "1610612736"
linux:
  resources:
    memory_limit_in_bytes: 1073741824
image:
  image: "$image"
command:
- top
EOF

	testContainerStart 1
	getContainerSwapInfo

	if [ $swappiness -ne 100 ]; then
		echo "The VM swappiness $swappiness with swap device is not right"
		false
	fi
	if [ $swap_in_bytes -ne 1610612736 ]; then
		echo "The VM swap_in_bytes $swap_in_bytes with swap device is not right"
		false
	fi
	if [ $swap_size -ne 536870912 ]; then
		echo "The VM swap size $swap_size with swap device is not right"
		false
	fi
}

@test "Test with enabled guest swap and container with memory_limit_in_bytes" {
	# Test without swap_in_bytes
	cat << EOF > "${container_yaml}"
metadata:
  name: busybox-swap
  namespace: default
  uid: busybox-swap-uid
annotations:
  io.katacontainers.container.resource.swappiness: "100"
linux:
  resources:
    memory_limit_in_bytes: 1073741824
image:
  image: "$image"
command:
- top
EOF

	testContainerStart 1
	getContainerSwapInfo

	if [ $swappiness -ne 100 ]; then
		echo "The VM swappiness $swappiness without swap_in_bytes is not right"
		false
	fi
	# swap_in_bytes is not set, it should be a value that bigger than 1125899906842624
	if [ $swap_in_bytes -lt 1125899906842624 ]; then
		echo "The VM swap_in_bytes $swap_in_bytes without swap_in_bytes is not right"
		false
	fi
	if [ $swap_size -ne 1073741824 ]; then
		echo "The VM swap size $swap_size without swap_in_bytes is not right"
		false
	fi
}

@test "Test with enabled guest swap and container without swap_in_bytes nor memory_limit_in_bytes" {
	# Test without memory_limit_in_bytes
	cat << EOF > "${container_yaml}"
metadata:
  name: busybox-swap
  namespace: default
  uid: busybox-swap-uid
annotations:
  io.katacontainers.container.resource.swappiness: "100"
image:
  image: "$image"
command:
- top
EOF

	testContainerStart 1
	getContainerSwapInfo

	if [ $swappiness -ne 100 ]; then
		echo "The VM swappiness $swappiness without memory_limit_in_bytes is not right"
		false
	fi
	# swap_in_bytes is not set, it should be a value that bigger than 1125899906842624
	if [ $swap_in_bytes -lt 1125899906842624 ]; then
		echo "The VM swap_in_bytes $swap_in_bytes without memory_limit_in_bytes is not right"
		false
	fi
	if [ $swap_size -ne 2147483648 ]; then
		echo "The VM swap size $swap_size without memory_limit_in_bytes is not right"
		false
	fi
}