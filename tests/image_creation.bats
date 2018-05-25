#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

readonly rootfs_sh="$BATS_TEST_DIRNAME/../rootfs-builder/rootfs.sh"
readonly image_builder_sh="$BATS_TEST_DIRNAME/../image-builder/image_builder.sh"
readonly initrd_builder_sh="$BATS_TEST_DIRNAME/../initrd-builder/initrd_builder.sh"
readonly tmp_dir=$(mktemp -t -d osbuilder-test.XXXXXXX)
readonly tmp_rootfs="${tmp_dir}/rootfs-osbuilder"
readonly osbuilder_file="/var/lib/osbuilder/osbuilder.yaml"

setup()
{
	export USE_DOCKER=true
}

teardown(){
	# Rootfs is own by root change it to remove it
	sudo rm -rf "${tmp_rootfs}"
	rm -rf "${tmp_dir}"
}

build_rootfs()
{
	local distro="$1"
	local rootfs="$2"

	local full="${rootfs}${osbuilder_file}"

	# clean up from any previous runs
	[ -d "${rootfs}" ] && sudo rm -rf "${rootfs}"

	sudo -E ${rootfs_sh} -r "${rootfs}" "${distro}"

	yamllint "${full}"
}

build_image()
{
	local file="$1"
	local rootfs="$2"

	sudo -E ${image_builder_sh} -o "${file}" "${rootfs}"
}

build_initrd()
{
	local file="$1"
	local rootfs="$2"

	sudo -E ${initrd_builder_sh} -o "${file}" "${rootfs}"
}

build_rootfs_image_initrd()
{
	distro="$1"
	image="$2"
	initrd="$3"

	[ -n "$distro" ]
	build_rootfs "${distro}" "${tmp_rootfs}"

	[ "$image" = "yes" ] && build_image "${tmp_dir}/image.img" "${tmp_rootfs}"
	[ "$initrd" = "yes" ] && build_initrd "${tmp_dir}/initrd-image.img" "${tmp_rootfs}"
}

@test "Can create fedora image" {
	build_rootfs_image_initrd fedora yes yes
}

@test "Can create clearlinux image" {
	build_rootfs_image_initrd clearlinux yes yes
}

@test "Can create centos image" {
	build_rootfs_image_initrd centos yes yes
}

@test "Can create euleros image" {
	if [ "$TRAVIS" = true ]
	then
		skip "travis timeout, see: https://github.com/kata-containers/osbuilder/issues/46"
	fi
	build_rootfs_image_initrd euleros yes yes
}

@test "Can create alpine image" {
	build_rootfs_image_initrd alpine no yes
}
