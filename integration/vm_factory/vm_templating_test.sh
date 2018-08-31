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
}

disable_vm_template_config() {
	echo "disable vm template config"
	sudo sed -i -e 's/^\(enable_template\).*=.*$/#\1 = true/g' "${RUNTIME_CONFIG_PATH}"
}

init_vm_template() {
	echo "init vm template"
	sudo -E PATH=$PATH "$kata_runtime_bin" factory init
}

destroy_vm_template() {
	echo "destroy vm template"
	sudo -E PATH=$PATH "$kata_runtime_bin" factory destroy
	# verify template is destroied
	res=$(mount | grep template | wc -l)
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

setup() {
	clean_env
	extract_kata_env
}

check_qemu_for_vm_template() {
	echo "checking qemu command line for vm template arguments"
	ps aux | grep ${HYPERVISOR_PATH} | grep ${template_tmpfs_path}
	[ $? -eq 0 ] || die "vm is not backed by vm template"
}

test_create_container_with_vm_template() {
	sudo docker run --runtime=$RUNTIME -d --name $CONTAINER_NAME $IMAGE $PAYLOAD_ARGS
	check_qemu_for_vm_template
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

echo "Ending vm templating test"
teardown
