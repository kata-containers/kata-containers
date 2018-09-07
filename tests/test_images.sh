#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

readonly script_dir="$(dirname $(readlink -f $0))"
readonly script_name=${0##*/}

readonly rootfs_sh="${script_dir}/../rootfs-builder/rootfs.sh"
readonly image_builder_sh="${script_dir}/../image-builder/image_builder.sh"
readonly initrd_builder_sh="${script_dir}/../initrd-builder/initrd_builder.sh"
readonly tmp_dir=$(mktemp -t -d osbuilder-test.XXXXXXX)
readonly tmp_rootfs="${tmp_dir}/rootfs-osbuilder"
readonly images_dir="${tmp_dir}/images"
readonly osbuilder_file="/var/lib/osbuilder/osbuilder.yaml"
readonly docker_image="busybox"
readonly docker_config_file="/etc/systemd/system/docker.service.d/kata-containers.conf"
readonly tests_repo="github.com/kata-containers/tests"
readonly tests_repo_dir="${script_dir}/../../tests"
readonly mgr="${tests_repo_dir}/cmd/kata-manager/kata-manager.sh"
readonly RUNTIME=${RUNTIME:-kata-runtime}
readonly MACHINE_TYPE=`uname -m`

# all distro tests must have this prefix
readonly test_func_prefix="test_distro_"

# "docker build" does not work with a VM-based runtime
readonly docker_build_runtime="runc"

test_images_only="false"
test_initrds_only="false"

# Hashes used to keep track of image sizes.
# - Key: name of distro.
# - Value: colon-separated roots and image sizes ("${rootfs_size}:${image_size}").
typeset -A built_images
typeset -A built_initrds

usage()
{
	cat <<EOT
Usage: $script_name [help|<distro>]
       $script_name [options]

Options:

  -h | --help          # Show usage.
  --distro <distro>    # Only run tests for specified distro.
  --list               # List all distros that can be tested.
  --test-images-only   # Only run images tests for the list of distros under test.
  --test-initrds-only  # Only run initrds tests for the list of distros under test.

Parameters:


help     : Show usage.
<distro> : Only run tests for specified distro.

Notes:

- If no options or parameters are specified, all tests will be run.

EOT
}

# Add an entry to the specified stats file
add_to_stats_file()
{
	local statsfile="$1"
	local name="$2"
	local entry="$3"
	local entry_type="$4"

	local rootfs_size_bytes
	local rootfs_size_mb

	local image_size_bytes
	local image_size_mb

	rootfs_size_bytes=$(echo "$entry"|cut -d: -f1)
	image_size_bytes=$(echo "$entry"|cut -d: -f2)

	rootfs_size_mb=$(bc <<< "scale=2; ${rootfs_size_bytes} / 2^20")
	image_size_mb=$(bc <<< "scale=2; ${image_size_bytes} / 2^20")

	printf '%12.12s\t%10.10s\t%12.12s\t%10.10s\t%-8.8s\t%-20.20s\n' \
		"${image_size_bytes}" \
		"${image_size_mb}" \
		"${rootfs_size_bytes}" \
		"${rootfs_size_mb}" \
		"${entry_type}" \
		"${name}" >> "$statsfile"
}

# Show the sizes of all the generated initrds and images
show_stats()
{
	local name
	local sizes

	local tmpfile=$(mktemp)

	# images
	for name in "${!built_images[@]}"
	do
		sizes=${built_images[$name]}
		add_to_stats_file "$tmpfile" "$name" "$sizes" 'image'
	done

	# initrds
	for name in "${!built_initrds[@]}"
	do
		sizes=${built_initrds[$name]}
		add_to_stats_file "$tmpfile" "$name" "$sizes" 'initrd'
	done

	info "Image and rootfs sizes (in bytes and MB), smallest image first:"
	echo

	printf '%12.12s\t%10.10s\t%12.12s\t%10.10s\t%-8.8s\t%-20.20s\n' \
		"image-bytes" \
		"image-MB" \
		"rootfs-bytes" \
		"rootfs-MB" \
		"Type" \
		"Name"

	sort -k1,1n "$tmpfile"

	rm -f "${tmpfile}"
}

exit_handler()
{
	if [ "$?" -eq 0 ]
	then
		info "tests passed successfully - cleaning up"

		# Rootfs and images are owned by root
		sudo -E rm -rf "${tmp_rootfs}"
		sudo -E rm -rf "${images_dir}"

		rm -rf "${tmp_dir}"

		return
	fi

	info "ERROR: test failed"

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

trap exit_handler EXIT ERR

die()
{
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

info()
{
	s="$*"
	echo -e "INFO: $s\n" >&2
}

set_runtime()
{
	local name="$1"

	[ -z "$name" ] && die "need name"

	# Travis doesn't support VT-x
	[ -n "$TRAVIS" ] && return

	sudo -E sed -i "s/--default-runtime=[^ ][^ ]*/--default-runtime=${name}/g" \
		"${docker_config_file}"
	sudo -E systemctl daemon-reload
	sudo -E systemctl restart docker
}

setup()
{
	[ -z "$images_dir" ] && die "need images directory"
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

build_rootfs()
{
	local distro="$1"
	local rootfs="$2"

	[ -z "$distro" ] && die "need distro"
	[ -z "$rootfs" ] && die "need rootfs"

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

	[ -z "$file" ] && die "need file"
	[ -z "$rootfs" ] && die "need rootfs"

	sudo -E ${image_builder_sh} -o "${file}" "${rootfs}"

	info "built image file '$file' for rootfs '$rootfs':"
	sudo -E ls -l "$file" >&2
}

build_initrd()
{
	local file="$1"
	local rootfs="$2"

	[ -z "$file" ] && die "need file"
	[ -z "$rootfs" ] && die "need rootfs"

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

	[ -z "$file" ] && die "need file"
	[ ! -e "$file" ] && die "file does not exist: $file"

	# Travis doesn't support VT-x
	[ -n "$TRAVIS" ] && return

	chronic $mgr reset-config
	chronic $mgr configure-image "$file"
	create_container
}

install_initrd_create_container()
{
	local file="$1"

	[ -z "$file" ] && die "need file"
	[ ! -e "$file" ] && die "file does not exist: $file"

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

	[ -z "$distro" ] && die "need distro"
	[ -z "$type" ] && die "need type"

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

		local rootfs_size=$(du -sb "${rootfs}" | awk '{print $1}')

		if [ "$type" = "image" ]
		then
			# Images need systemd
			[ "$opt" = "init" ] && continue

			local image_path="${images_dir}/${type}-${distro}-agent-init-${AGENT_INIT}.img"

			build_image "${image_path}" "${rootfs}"
			local image_size=$(stat -c "%s" "${image_path}")

			built_images["${distro}"]="${rootfs_size}:${image_size}"

			install_image_create_container "${image_path}"
		elif [ "$type" = "initrd" ]
		then
			local initrd_path="${images_dir}/${type}-${distro}-agent-init-${AGENT_INIT}.img"

			build_initrd "${initrd_path}" "${rootfs}"
			local initrd_size=$(stat -c "%s" "${initrd_path}")

			built_initrds["${distro}"]="${rootfs_size}:${initrd_size}"

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

	[ -z "$distro" ] && die "need distro"
	[ -z "$image_options" ] && die "need image options"
	[ -z "$initrd_options" ] && die "need initrd options"

	local opt

	if [ "$image_options" != "no" ]
	then
		if [ "${test_initrds_only}" = "true" ]
		then
			info "only testing initrds: skipping image test for distro $distro"
		else
			handle_options "$distro" "image" "$image_options"
		fi
	fi

	if [ "$initrd_options" != "no" ]
	then
		if [ "${test_images_only}" = "true" ]
		then
			info "only testing images: skipping initrd test for distro $distro"
		else
			handle_options "$distro" "initrd" "$initrd_options"
		fi
	fi
}

run_test()
{
	local -r name="$1"
	local -r skip="$2"
	local -r distro="$3"
	local -r image_options="$4"
	local -r initrd_options="$5"

	[ -z "$name" ] && die "need name"
	[ -z "$distro" ] && die "need distro"
	[ -z "$image_options" ] && die "need image options"
	[ -z "$initrd_options" ] && die "need initrd options"

	[ -n "$skip" ] && info "Skipping test $name: $skip" && return

	info "Running test: ${name}"

	create_and_run "${distro}" "${image_options}" "${initrd_options}"
}

test_distro_ubuntu()
{
       local -r name="Can create and run ubuntu image"
       run_test "${name}" "" "ubuntu" "service" "no"
}


test_distro_fedora()
{
	local -r name="Can create and run fedora image"
	run_test "${name}" "" "fedora" "service" "no"
}

test_distro_clearlinux()
{
	local -r name="Can create and run clearlinux image"

	run_test "${name}" "" "clearlinux" "service" "no"
}

test_distro_centos()
{
	local -r name="Can create and run centos image"
	run_test "${name}" "" "centos" "service" "no"
}

test_distro_euleros()
{
	local -r name="Can create and run euleros image"

	[ "$TRAVIS" = true ] && skip="travis timeout, see: https://github.com/kata-containers/osbuilder/issues/46"

	run_test "${name}" "$skip" "euleros" "service" "no"
}

test_distro_alpine()
{
	local -r name="Can create and run alpine image"
	run_test "${name}" "" "alpine" "no" "init"
}

# Displays a list of all distro test functions
get_distro_test_names()
{
	typeset -F | awk '{print $3}' |\
		grep "^${test_func_prefix}" | sort
}

# Displays a list of distros which can be tested
list_distros()
{
	get_distro_test_names | sed "s/${test_func_prefix}//g"
}

test_single_distro()
{
	local -r distro="$1"

	[ -z "$distro" ] && die "distro cannot be blank"

	local -r expected_func="${test_func_prefix}${distro}"

	local test_funcs
	test_funcs=$(get_distro_test_names)

	local defined_func
	defined_func=$(echo "$test_funcs" | grep "^${expected_func}$" || true)

	if [ -z "$defined_func" ]
	then
		local distros

		# make a comma-separated list
		distros=$(list_distros | tr '\n' ',' | sed 's/,$//g')

		die "no test for distro '$distro' (try one of $distros)"
	fi

	info "only running tests for distro $distro"

	# run the test
	$defined_func
}

test_all_distros()
{
	info "running tests for all distros"

	test_distro_fedora
	test_distro_centos
	test_distro_alpine
	test_distro_ubuntu
	if [ $MACHINE_TYPE != "ppc64le" ]; then
	   test_distro_clearlinux

	   # Run last as EulerOS servers can be slow and we don't want to fail the
	   # previous tests.
	   test_distro_euleros
	fi

	show_stats
}

main()
{
	local args=$(getopt \
		-n "$script_name" \
		-a \
		--options="h" \
		--longoptions="help distro: list test-images-only test-initrds-only" \
		-- "$@")

	eval set -- "$args"
	[ $? -ne 0 ] && { usage >&2; exit 1; }

	local distro=

	while [ $# -gt 1 ]
	do
		case "$1" in
			--distro) distro="$2";;

			-h|--help) usage; exit 0 ;;

			--list) list_distros; exit 0;;

			--test-images-only)
				test_images_only="true"
				test_initrds_only="false"
				;;

			--test-initrds-only)
				test_initrds_only="true"
				test_images_only="false"
				;;

			--) shift; break ;;
		esac

		shift
	done

	# Consume getopt cruft
	[ "$1" = "--" ] && shift

	case "$1" in
		help) usage && exit 0;;
		*) distro="$1";;
	esac

	setup

	if [ -n "$distro" ]
	then
		test_single_distro "$distro"
	else
		test_all_distros
	fi

	# We shouldn't really need a message like this but the CI can fail in
	# mysterious ways so make it clear!
	info "all tests finished successfully"
}

main "$@"
