#!/bin/bash
#
# Copyright (c) 2018 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will perform several tests to validate kata containers with
# vm templating.

set -e -x

cidir=$(dirname "$0")

source "${cidir}/../../metrics/lib/common.bash"

# Environment variables
IMAGE="${IMAGE:-busybox}"
CONTAINER_NAME=$(random_name)
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"

kata_runtime_bin=$(command -v kata-runtime)

template_tmpfs_path="/run/vc/vm/template"

enable_vm_template_config() {
	echo "enable vm template config ${RUNTIME_CONFIG_PATH}"
	sudo sed -i -e 's/^#\(enable_template\).*=.*$/\1 = true/g' "${RUNTIME_CONFIG_PATH}"
	sudo sed -i -e 's/^#\(use_vsock\).*=.*$/\1 = true/g' "${RUNTIME_CONFIG_PATH}"
}

disable_vm_template_config() {
	echo "disable vm template config"
	sudo sed -i -e 's/^\(enable_template\).*=.*$/#\1 = true/g' "${RUNTIME_CONFIG_PATH}"
	sudo sed -i -e 's/^\(use_vsock\).*=.*$/#\1 = true/g' "${RUNTIME_CONFIG_PATH}"
}

init_vm_template() {
	echo "init vm template"
	sudo -E PATH=$PATH "$kata_runtime_bin" factory init

	{ sudo -E PATH=$PATH "$kata_runtime_bin" factory init ; res=$?; } || true
	[ $res -ne 0 ] || die "factory init already called so expected 2nd call to fail"
}

destroy_vm_template() {
	echo "destroy vm template"
	sudo -E PATH=$PATH "$kata_runtime_bin" factory destroy
	# verify template is destroied
	res=$(mount | grep ${template_tmpfs_path} | wc -l)
	[ $res -eq 0 ] || die "template factory is not cleaned up"
}

check_vm_template_factory() {
	echo "checking vm template factory"
	mount | grep tmpfs | grep ${template_tmpfs_path}
	res=$(sudo ls ${template_tmpfs_path} | grep memory |wc -l)
	[ $res -eq 1 ] || die "template factory is not set up, missing memory file"
	res=$(sudo ls ${template_tmpfs_path} | grep state |wc -l)
	[ $res -eq 1 ] || die "template factory is not set up, missing state file"
}

clean_storage_in_dir() {
	for file in $(ls $1); do
		sudo rm -rf $1/$file
	done
}

clean_storage() {
	clean_storage_in_dir ${VC_POD_DIR}
	clean_storage_in_dir ${RUN_SBS_DIR}
}

check_storage_leak() {
	check_pods_in_dir ${VC_POD_DIR}
	check_pods_in_dir ${RUN_SBS_DIR}
}

clean_vm_template() {
	while [ $(mount | grep ${template_tmpfs_path} | wc -l) -ne 0 ]
	do
		echo "cleaning vm template"
		{ umount ${template_tmpfs_path} ; res=$?; } || true
		[ $res -eq 0 ] || die "clean vm template failed"
	done
}

setup() {
	clean_env
	extract_kata_env
	clean_storage
	clean_vm_template
}

check_qemu_for_vm_template() {
	echo "checking qemu command line for vm template arguments"
	ps aux | grep ${HYPERVISOR_PATH} | grep ${template_tmpfs_path}
	[ $? -eq 0 ] || die "vm is not backed by vm template"
}

check_vm_template_network_setup() {
	IPADDR=$(sudo docker exec -t $CONTAINER_NAME ip addr show eth0| sed -ne "s|.*inet \(.*\)/.* brd .*|\1|p")
	[[ ${IPADDR} =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "vm eth0 ip is ${IPADDR}"
}

check_new_guest_date_time() {
	HOSTTIME=$(date +%s)
	GUESTTIME=$(sudo docker exec $CONTAINER_NAME date +%s)
	[[ ${HOSTTIME} -le ${GUESTTIME} ]] || die "hosttime ${HOSTTIME} guesttime ${GUESTTIME}"
}

test_create_container_with_vm_template() {
	# sleep a bit so that template VM time is in the past
	sleep 2
	sudo docker run --runtime=$RUNTIME -d --name $CONTAINER_NAME $IMAGE $PAYLOAD_ARGS
	check_qemu_for_vm_template
	check_vm_template_network_setup
	check_new_guest_date_time
	sudo docker rm -f $CONTAINER_NAME
}

test_factory_init_destroy() {
	echo "test kata-runtime factory init destroy"
	enable_vm_template_config
	init_vm_template
	check_vm_template_factory
	test_create_container_with_vm_template
	destroy_vm_template
	disable_vm_template_config
}

test_docker_create_auto_init_vm_factory() {
	echo "test docker create auto init vm factory"
	enable_vm_template_config
	sudo docker run --runtime=$RUNTIME -d --name $CONTAINER_NAME $IMAGE $PAYLOAD_ARGS
	check_vm_template_factory
	check_qemu_for_vm_template
	check_vm_template_network_setup
	sudo docker rm -f $CONTAINER_NAME
	destroy_vm_template
	disable_vm_template_config
}

teardown() {
	clean_env
}

if [ -z $INITRD_PATH ]; then
	echo "Skipping vm templating test as initrd is not set"
	exit 0
fi

echo "Starting vm templating test"
setup

echo "Running vm templating test"
test_factory_init_destroy
test_docker_create_auto_init_vm_factory

echo "check storage leak after vm templating test"
check_storage_leak

echo "Ending vm templating test"
teardown
