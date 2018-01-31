#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0


rootfs_sh="$BATS_TEST_DIRNAME/../rootfs-builder/rootfs.sh"
image_builder_sh="$BATS_TEST_DIRNAME/../image-builder/image_builder.sh"
initrd_builder_sh="$BATS_TEST_DIRNAME/../initrd-builder/initrd_builder.sh"
readonly tmp_dir=$(mktemp -t -d osbuilder-test.XXXXXXX)
tmp_rootfs="${tmp_dir}/rootfs-osbuilder"
#FIXME: Remove image size after https://github.com/kata-containers/osbuilder/issues/25 is fixed
readonly image_size=400


setup()
{
	export USE_DOCKER=true
}

teardown(){
	# Rootfs is own by root change it to remove it
	sudo rm -rf "${tmp_rootfs}"
	rm -rf "${tmp_dir}"
}

function build_rootfs()
{
	sudo -E ${rootfs_sh} -r "${tmp_rootfs}" "${distro}"
}

function build_image()
{
	sudo -E ${image_builder_sh} -s ${image_size} -o "${tmp_dir}/image.img" "${tmp_rootfs}"
}

function build_initrd()
{
	sudo -E ${initrd_builder_sh} -o "${tmp_dir}/initrd-image.img" "${tmp_rootfs}"
}

function build_rootfs_image_initrd()
{
	distro="$1"
	[ -n "$distro" ]
	build_rootfs $distro
	build_image
	build_initrd
}

@test "Can create fedora image" {
	build_rootfs_image_initrd fedora
}

@test "Can create clearlinux image" {
	build_rootfs_image_initrd clearlinux
}

@test "Can create centos image" {
	build_rootfs_image_initrd centos
}

@test "Can create euleros image" {
	build_rootfs_image_initrd euleros
}

@test "Can create alpine image" {
	build_rootfs_image_initrd alpine
}
