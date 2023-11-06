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

testContainerMemoryUpdate() {
	test_virtio_mem=$1

	if [ $test_virtio_mem -eq 1 ]; then
		sudo sed -i -e 's/^#enable_virtio_mem.*$/enable_virtio_mem = true/g' "${kata_config}"
	else
		sudo sed -i -e 's/^enable_virtio_mem.*$/#enable_virtio_mem = true/g' "${kata_config}"
	fi

	testContainerStart

	vm_size=$(($(sudo crictl exec $cid cat /proc/meminfo | grep "MemTotal:" | awk '{print $2}')*1024))
	if [ $vm_size -gt $((2*1024*1024*1024)) ] || [ $vm_size -lt $((2*1024*1024*1024-128*1024*1024)) ]; then
		testContainerStop
		echo "The VM memory size $vm_size before update is not right"
		return 1
	fi

	sudo crictl update --memory $((2*1024*1024*1024)) $cid
	sleep 1

	vm_size=$(($(sudo crictl exec $cid cat /proc/meminfo | grep "MemTotal:" | awk '{print $2}')*1024))
	if [ $vm_size -gt $((4*1024*1024*1024)) ] || [ $vm_size -lt $((4*1024*1024*1024-128*1024*1024)) ]; then
		testContainerStop
		echo "The VM memory size $vm_size after increase is not right"
		return 1
	fi

	if [ $test_virtio_mem -eq 1 ]; then
		sudo crictl update --memory $((1*1024*1024*1024)) $cid
		sleep 1

		vm_size=$(($(sudo crictl exec $cid cat /proc/meminfo | grep "MemTotal:" | awk '{print $2}')*1024))
		if [ $vm_size -gt $((3*1024*1024*1024)) ] || [ $vm_size -lt $((3*1024*1024*1024-128*1024*1024)) ]; then
			testContainerStop
			echo "The VM memory size $vm_size after decrease is not right"
			return 1
		fi
	fi

	testContainerStop
}

setup() {
	if [[ "${KATA_HYPERVISOR}" != "qemu" ]] || [[ "${ARCH}" == "ppc64le" ]] || [[ "${ARCH}" == "s390x" ]]; then
		skip "Test not supported on $KATA_HYPERVISOR $ARCH"
	fi

	sudo cp "$default_containerd_config" "$default_containerd_config_backup"
	sudo cp "$CONTAINERD_CONFIG_FILE" "$default_containerd_config"

	restart_containerd_service

	[ -f "$kata_config" ] && sudo cp "$kata_config" "$kata_config_backup" || \
		sudo cp "$default_kata_config" "$kata_config"
}

@test "Test container memory update without virtio-mem" {
	testContainerMemoryUpdate 0
}

@test "Test container memory update with virtio-mem" {
	if [[ "$ARCH" != "x86_64" ]]; then
		skip "Test only supported on x86_64"
	fi

	testContainerMemoryUpdate 1
}

teardown() {
	sudo cp "$default_containerd_config_backup" "$default_containerd_config"
	restart_containerd_service

	[ -f "$kata_config_backup" ] && sudo mv "$kata_config_backup" "$kata_config" || \
		sudo rm "$kata_config"
}