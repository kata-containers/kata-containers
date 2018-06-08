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
readonly images_dir="${tmp_dir}/images"
readonly osbuilder_file="/var/lib/osbuilder/osbuilder.yaml"
readonly docker_image="busybox"
readonly docker_config_file="/etc/systemd/system/docker.service.d/kata-containers.conf"
readonly tests_repo="github.com/kata-containers/tests"
readonly tests_repo_dir="$BATS_TEST_DIRNAME/../../tests"
readonly mgr="${tests_repo_dir}/cmd/kata-manager/kata-manager.sh"
readonly RUNTIME=${RUNTIME:-kata-runtime}

# "docker build" does not work with a VM-based runtime
readonly docker_build_runtime="runc"

info()
{
	s="$*"
	echo -e "INFO: $s\n" >&2
}

set_runtime()
{
	local name="$1"

	# Travis doesn't support VT-x
	[ -n "$TRAVIS" ] && return

	sudo -E sed -i "s/--default-runtime=[^ ][^ ]*/--default-runtime=${name}/g" \
		"${docker_config_file}"
	sudo -E systemctl daemon-reload
	sudo -E systemctl restart docker
}

setup()
{
	mkdir -p "${images_dir}"

	export USE_DOCKER=true

	# Travis doesn't support VT-x
	[ -n "$TRAVIS" ] && return

	[ ! -d "${tests_repo_dir}" ] && git clone "https://${tests_repo}" "${tests_repo_dir}"

	chronic $mgr install-docker-system
	chronic $mgr enable-debug

	# Ensure "docker build" works
	set_runtime "${docker_build_runtime}"
}

teardown()
{
	if [ "$BATS_ERROR_STATUS" -eq 0 ]
	then
		# Rootfs and images are owned by root
		sudo -E rm -rf "${tmp_rootfs}"
		sudo -E rm -rf "${images_dir}"

		rm -rf "${tmp_dir}"

		return
	fi

	# The test failed so dump what we can

	info "AGENT_INIT: '${AGENT_INIT}'"

	info "images:"
	sudo -E ls -l "${images_dir}" >&2

	info "rootfs:"
	sudo -E ls -l "${tmp_rootfs}" >&2

	info "local runtime config:"
	cat /etc/kata-containers/configuration.toml >&2

	info "main runtime config:"
	cat /usr/share/defaults/kata-containers/configuration.toml >&2

	info "collect script output:"
	sudo -E kata-collect-data.sh >&2

	info "processes:"
	sudo -E ps -efwww | egrep "docker|kata" >&2
}

build_rootfs()
{
	local distro="$1"
	local rootfs="$2"

	local full="${rootfs}${osbuilder_file}"

	# clean up from any previous runs
	[ -d "${rootfs}" ] && sudo -E rm -rf "${rootfs}"

	sudo -E ${rootfs_sh} -r "${rootfs}" "${distro}"

	yamllint "${full}"

	info "built rootfs for distro '$distro' at '$rootfs'"
	info "osbuilder metadata file:"
	cat "${full}" >&2
}

build_image()
{
	local file="$1"
	local rootfs="$2"

	sudo -E ${image_builder_sh} -o "${file}" "${rootfs}"

	info "built image file '$file' for rootfs '$rootfs':"
	sudo -E ls -l "$file" >&2
}

build_initrd()
{
	local file="$1"
	local rootfs="$2"

	sudo -E ${initrd_builder_sh} -o "${file}" "${rootfs}"

	info "built initrd file '$file' for rootfs '$rootfs':"
	sudo -E ls -l "$file" >&2
}

create_container()
{
	out=$(mktemp)

	local file="/proc/version"

	# Create a container using the runtime under test which displays a
	# file that is expected to exist.
	docker run --rm -i --runtime "${RUNTIME}" "$docker_image" cat "${file}" > "$out"

	info "contents of docker image ${docker_image} container file '${file}':"
	cat "${out}" >&2

	[ -s "$out" ]
	rm -f "$out"
}

install_image_create_container()
{
	local file="$1"

	# Travis doesn't support VT-x
	[ -n "$TRAVIS" ] && return

	chronic $mgr reset-config
	chronic $mgr configure-image "$file"
	create_container
}

install_initrd_create_container()
{
	local file="$1"

	# Travis doesn't support VT-x
	[ -n "$TRAVIS" ] && return

	chronic $mgr reset-config
	chronic $mgr configure-initrd "$file"
	create_container
}

handle_options()
{
	local distro="$1"
	local type="$2"
	local options="$3"

	local opt
	local rootfs

	for opt in $options
	do
		# Set the crucial variable to determine if the agent will be
		# PID 1 in the image or initrd.
		case "$opt" in
			init) export AGENT_INIT="yes";;
			*) export AGENT_INIT="no";;
		esac

		rootfs="${tmp_rootfs}/${distro}-agent-init-${AGENT_INIT}"

		build_rootfs "${distro}" "${rootfs}"

		if [ "$type" = "image" ]
		then
			# Images need systemd
			[ "$opt" = "init" ] && continue

			local image_path="${images_dir}/${type}-${distro}-agent-init-${AGENT_INIT}.img"

			build_image "${image_path}" "${rootfs}"
			install_image_create_container "${image_path}"
		elif [ "$type" = "initrd" ]
		then
			local initrd_path="${images_dir}/${type}-${distro}-agent-init-${AGENT_INIT}.img"

			build_initrd "${initrd_path}" "${rootfs}"
			install_initrd_create_container "${initrd_path}"
		else
			die "invalid type: '$type' for distro $distro option $opt"
		fi
	done
}

# Create an image and/or initrd for the specified distribution,
# then test each by configuring the runtime and creating a container.
#
# The second and third parameters take the form of a space separated list of
# values which represent whether the agent should be the init daemon in the
# image/initrd. "init" means the agent should be configured to be the init
# daemon and "service" means it should run as a systemd service.
#
# The list value should be set to "no" if the image/initrd should not
# be built+tested.
#
# Parameters:
#
# 1: distro name.
# 2: image options.
# 3: initrd options.
create_and_run()
{
	local distro="$1"
	local image_options="$2"
	local initrd_options="$3"

	[ -n "$distro" ]

	local opt

	if [ "$image_options" != "no" ]
	then
		handle_options "$distro" "image" "$image_options"
	fi

	if [ "$initrd_options" != "no" ]
	then
		handle_options "$distro" "initrd" "$initrd_options"
	fi
}

@test "Can create and run fedora image" {
	create_and_run fedora "service" "no"
}

@test "Can create and run clearlinux image" {
	create_and_run clearlinux "service" "no"
}

@test "Can create and run centos image" {
	create_and_run centos "service" "no"
}

@test "Can create and run euleros image" {
	if [ "$TRAVIS" = true ]
	then
		skip "travis timeout, see: https://github.com/kata-containers/osbuilder/issues/46"
	fi

	create_and_run euleros "service" "no"
}

@test "Can create and run alpine image" {
	create_and_run alpine "no" "init"
}
