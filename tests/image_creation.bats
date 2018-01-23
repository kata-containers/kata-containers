#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0


rootfs_sh="$BATS_TEST_DIRNAME/../rootfs-builder/rootfs.sh"
image_builder_sh="$BATS_TEST_DIRNAME/../image-builder/image_builder.sh"
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

function build_image()
{
	distro="$1"
	[ -n "$distro" ]
	local rootfs="${tmp_dir}/rootfs-osbuilder"
	sudo -E ${rootfs_sh} -r "${rootfs}" "${distro}"
	sudo ${image_builder_sh} -s ${image_size} -o "${tmp_dir}/image.img" "${rootfs}"
}

@test "Can create fedora image" {
	build_image fedora
}

@test "Can create clearlinux image" {
	build_image clearlinux
}

@test "Can create centos image" {
	build_image centos 
}

@test "Can create euleros image" {
	if [ "$TRAVIS" = true ]
	then
		skip "travis timout, see: https://github.com/kata-containers/osbuilder/issues/46"
	fi
	build_image euleros
}
