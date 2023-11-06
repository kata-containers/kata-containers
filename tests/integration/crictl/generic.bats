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

setup() {
	sudo cp "$default_containerd_config" "$default_containerd_config_backup"
	sudo cp "$CONTAINERD_CONFIG_FILE" "$default_containerd_config"

	restart_containerd_service
	testContainerStart
}

@test "Test killed vmm cleanup" {
	if [[ "${KATA_HYPERVISOR}" != "qemu" ]]; then
		skip "Skipped for ${KATA_HYPERVISOR}, only QEMU is currently tested"
	fi

	qemu_pid=$(ps aux|grep qemu|grep -v grep|awk '{print $2}')
	echo "kill qemu $qemu_pid"
	sudo kill -SIGKILL "$qemu_pid"
	# sleep to let shimv2 exit
	sleep 1
	echo "The shimv2 process should be killed"
	remained=$(ps aux|grep shimv2|grep -v grep || true)
	[ -z $remained ]
}

teardown() {
	testContainerStop

	sudo cp "$default_containerd_config_backup" "$default_containerd_config"
	restart_containerd_service
}
