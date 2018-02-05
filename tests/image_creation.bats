#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0


rootfs_sh="$BATS_TEST_DIRNAME/../rootfs-builder/rootfs.sh"
image_builder_sh="$BATS_TEST_DIRNAME/../image-builder/image_builder.sh"
initrd_builder_sh="$BATS_TEST_DIRNAME/../initrd-builder/initrd_builder.sh"
readonly tmp_dir=$(mktemp -t -d osbuilder-test.XXXXXXX)
#FIXME: Remove image size after https://github.com/kata-containers/osbuilder/issues/25 is fixed
readonly image_size=400


setup()
{
	export USE_DOCKER=true
}

teardown(){
	# Rootfs is own by root change it to remove it
	sudo rm -rf "${tmp_dir}/rootfs-osbuilder"
	rm -rf "${tmp_dir}"
}

function build_rootfs()
{
	distro="$1"
	[ -n "$distro" ]
	local rootfs="${tmp_dir}/rootfs-osbuilder"
	sudo -E ${rootfs_sh} -r "${rootfs}" "${distro}"
}

function build_image()
{
	distro="$1"
	[ -n "$distro" ]
	local rootfs="${tmp_dir}/rootfs-osbuilder"
	sudo -E ${image_builder_sh} -s ${image_size} -o "${tmp_dir}/image.img" "${rootfs}"
}

function build_initrd()
{
	distro="$1"
	[ -n "$distro" ]
	local rootfs="${tmp_dir}/rootfs-osbuilder"
	sudo -E ${initrd_builder_sh} -o "${tmp_dir}/initrd-image.img" "${rootfs}"
}

@test "Can create fedora image" {
	build_rootfs fedora
	build_image fedora
	build_initrd fedora
}

@test "Can create clearlinux image" {
	build_rootfs clearlinux
	build_image clearlinux
	build_initrd clearlinux
}

@test "Can create centos image" {
	build_rootfs centos
	build_image centos
	build_initrd centos
}

@test "Can create euleros image" {
	build_rootfs euleros
	build_image euleros
	build_initrd euleros
}
