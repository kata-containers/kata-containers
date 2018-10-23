#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

readonly script_dir="$(dirname $(readlink -f $0))"
readonly script_name=${0##*/}
readonly tmp_dir=$(mktemp -t -d osbuilder-test.XXXXXXX)
readonly tmp_rootfs="${tmp_dir}/rootfs-osbuilder"
readonly images_dir="${tmp_dir}/images"
readonly osbuilder_file="/var/lib/osbuilder/osbuilder.yaml"
readonly docker_image="busybox"
readonly systemd_docker_config_file="/etc/systemd/system/docker.service.d/kata-containers.conf"
readonly sysconfig_docker_config_file="/etc/sysconfig/docker"
readonly tests_repo="github.com/kata-containers/tests"
readonly tests_repo_dir="${script_dir}/../../tests"
readonly mgr="${tests_repo_dir}/cmd/kata-manager/kata-manager.sh"
readonly test_config=${script_dir}/test_config.sh
readonly rootfs_builder=${script_dir}/../rootfs-builder/rootfs.sh
readonly RUNTIME=${RUNTIME:-kata-runtime}
readonly MACHINE_TYPE=`uname -m`

# all distro tests must have this prefix
readonly test_func_prefix="test_distro_"

# "docker build" does not work with a VM-based runtime
readonly docker_build_runtime="runc"

build_images=1
build_initrds=1
typeset -a distrosSystemd distrosAgent
source ${test_config}
# Hashes used to keep track of image sizes.
# - Key: name of distro.
# - Value: colon-separated roots and image sizes ("${rootfs_size}:${image_size}").
typeset -A built_images
typeset -A built_initrds

usage()
{
	cat <<EOT
Usage: $script_name [options] [command | <distro>]

Options:
  -h | --help          # Show usage.
  --list               # List all distros that can be tested.
  --test-images-only   # Only run images tests for the list of distros under test.
  --test-initrds-only  # Only run initrds tests for the list of distros under test.

Commands:
help     : Show usage.


When <distro> is specified, tests are run only for the specified <distro>.
Otherwise, tests are run on all distros.

$(basename ${test_config}) includes a list of distros to exclude from testing,
depending on the detected test environment. However, when a <distro> is specified,
distro exclusion based on $(basename ${test_config}) is not enforced.
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

	sort -k1,1n -k3,3n "$tmpfile"

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

		# Restore the default image in config file
		[ -n "${TRAVIS:-}" ] || chronic $mgr configure-image

		return
	fi

	info "ERROR: test failed"

	# The test failed so dump what we can
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

	# Restore the default image in config file
	[ -n "${TRAVIS:-}" ] || chronic $mgr configure-image
}

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

debug()
{
	[ -z "${TEST_DEBUG:-}" ] && return
	s="$*"
	echo -e "DBG: $s" >&2
}


set_runtime()
{
	local name="$1"

	[ -z "$name" ] && die "need name"

	# Travis doesn't support VT-x
	[ -n "${TRAVIS:-}" ] && return

	if [ -f "$sysconfig_docker_config_file" ]; then
		docker_config_file="$sysconfig_docker_config_file"
		sed_script="s|^( *DOCKER_OPTS=.+--default-runtime[= ] *)[^ \"]+(.*\"$)|\1${name}\2|g"
	else
		docker_config_file="$systemd_docker_config_file"
		sed_script="s/--default-runtime[= ][^ ]*/--default-runtime=${name}/g"
	fi

	sudo -E sed -i -E "$sed_script" "$docker_config_file"
	sudo -E systemctl daemon-reload
	sudo -E systemctl restart docker
}

setup()
{
	[ -z "$images_dir" ] && die "need images directory"
	mkdir -p "${images_dir}"

	export USE_DOCKER=true

	# Travis doesn't support VT-x
	[ -n "${TRAVIS:-}" ] && return

	[ ! -d "${tests_repo_dir}" ] && git clone "https://${tests_repo}" "${tests_repo_dir}"

	chronic $mgr install-docker-system
	chronic $mgr enable-debug

	# Ensure "docker build" works
	set_runtime "${docker_build_runtime}"
}

# Fetches the distros test configuration from the distro-specific config.sh file.
# $1 : only fetch configuration for the distro with name $1. When not specified,
# fetch configuration for all distros.
get_distros_config()
{
	local distro="$1"
	local distrosList
	local -A distroCfg=(\
		[INIT_PROCESS]=\
		[ARCH_EXCLUDE_LIST]=\
		)

	if [ -n "$distro" ]; then
		distrosList=("$distro")
		# When specifying a single distro name, skip does not apply
		skipWhenTestingAll=()
	else
		distrosList=($(make list-distros))
	fi

	for d in ${distrosList[@]}; do
		debug "Getting config for distro $d"
		distroPattern="\<${d}\>"
		if [[ "${skipWhenTestingAll[@]}" =~ $distroPattern ]]; then
			info "Skipping distro $d as specified by $(basename ${test_config})"
			continue
		fi

		tmpfile=$(mktemp /tmp/osbuilder-$d-config.XXX)
		${rootfs_builder} -t $d  > $tmpfile
		# Get value of all keys in distroCfg
		for k in ${!distroCfg[@]}; do
			distroCfg[$k]="$(awk -v cfgKey=$k 'BEGIN{FS=":\t+"}{if ($1 == cfgKey) print $2}' $tmpfile)"
			debug "distroCfg[$k]=${distroCfg[$k]}"
		done
		rm -f $tmpfile

		machinePattern="\<${MACHINE_TYPE}\>"
		if [[ "${distroCfg[ARCH_EXCLUDE_LIST]}" =~ $machinePattern ]]; then
			info "Skipping distro $d on architecture $MACHINE_TYPE"
			continue
		fi

		case "${distroCfg[INIT_PROCESS]}" in
			systemd)    distrosSystemd+=($d) ;;
			kata-agent) distrosAgent+=($d) ;;
			*)			die "Invalid init process specified for distro $d: \"${distroCfg[INIT_PROCESS]}\"" ;;
		esac
	done
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
	[ -n "${TRAVIS:-}" ] && return

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
	[ -n "${TRAVIS:-}" ] && return

	chronic $mgr reset-config
	chronic $mgr configure-initrd "$file"
	create_container
}

# Displays a list of distros which can be tested
list_distros()
{
	tr " " "\n" <<< "${distrosSystemd[@]} ${distrosAgent[@]}" | sort
}

#
# Calls the `GNU make` utility with the set of passed arguments.
# Arguments can either be make targets or make variables assignments (in the form of VARIABLE=<value>)
#
call_make() {
	targetType=$1
	shift
	makeVars=()
	makeTargets=()
	# Split args between make variable and targets
	for t in $@; do
		# RE to match a make variable assignment
		pattern="^\w+\="
		if [[ "$t" =~ $pattern ]]; then
			makeVars+=("$t")
		else
			makeTargets+=($targetType-$t)
		fi
	done

	makeJobs=
	if [ -z "${CI:-}" ]; then
	  ((makeJobs=$(nproc) / 2))
	fi

	info "Starting make with \n\
	# of // jobs:  ${makeJobs:-[unlimited]} \n\
	targets:       ${makeTargets[@]} \n\
	variables:     ${makeVars[@]}"

	sudo -E make -j $makeJobs ${makeTargets[@]} ${makeVars[@]}
}

make_rootfs() {
	call_make rootfs $@
}

make_image() {
	call_make image $@
}

make_initrd() {
	call_make initrd $@
}

get_rootfs_size() {
	[ $# -ne 1 ] && die "get_rootfs_size with wrong invalid argument"

	local rootfs_dir=$1
	! [ -d "$rootfs_dir" ] && die "$rootfs_dir is not a valid rootfs path"

	sudo -E du -sb "${rootfs_dir}" | awk '{print $1}'
}

# Create an image and/or initrd for the available distributions,
# then test each by configuring the runtime and creating a container.
#
# When passing the name of a distribution, tests are run against that
# distribution only.
#
# Parameters:
#
# 1: distro name.
#
test_distros()
{
	local distro="$1"
	get_distros_config "$distro"
	local separator="~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~\n"

	echo -e "$separator"

	# If a distro was specified, filter out the distro list to only include that distro
	if [ -n "$distro" ]; then
		pattern="\<$distro\>"
		if [[ "${distrosAgent[@]}" =~ $pattern ]]; then
			distrosAgent=($distro)
			distrosSystemd=()
		elif [[ "${distrosSystemd[@]}" =~ $pattern ]]; then
			distrosSystemd=($distro)
			distrosAgent=()
			build_initrds=
		else
			die "Not a valid distro: $distro"
		fi

		info "Running tests for distro: $distro"

	else
		info "Running tests for all distros"
	fi

	# distro with systemd as init    -> normal rootfs image
	# distro with kata-agent as init -> normal rootfs image AND initrd image

	# If user does not need rootfs images, then do not build systemd rootfses
	[ -z "$build_images" ] && distrosSystemd=()

	commonMakeVars=( \
		USE_DOCKER=true \
		ROOTFS_BUILD_DEST="$tmp_rootfs" \
		IMAGES_BUILD_DEST="$images_dir" )

	# Build systemd and agent rootfs with 2 separate jobs
	bgJobs=()

	if [ ${#distrosSystemd[@]} -gt 0 ]; then
	  info "building rootfses with systemd as init: ${distrosSystemd[@]}"
	  make_rootfs ${commonMakeVars[@]} "${distrosSystemd[@]}" &
	  bgJobs+=($!)
	fi

	if [ ${#distrosAgent[@]} -gt 0 ]; then
	  info "building all rootfses with kata-agent as init"
	  make_rootfs ${commonMakeVars[@]} AGENT_INIT=yes "${distrosAgent[@]}" &
	  bgJobs+=($!)
	fi

	# Check for build failures (`wait` remembers up to CHILD_MAX bg processes exit status)
	for j in ${bgJobs[@]}; do
		wait $j || die "Background build job failed"
	done


	for d in ${distrosSystemd[@]} ${distrosAgent[@]}; do
		local rootfs_path="${tmp_rootfs}/${d}_rootfs"
		osbuilder_file_fullpath="${rootfs_path}/${osbuilder_file}"
		echo -e "$separator"
		yamllint "${osbuilder_file_fullpath}"

		info "osbuilder metadata file for $d:"
		cat "${osbuilder_file_fullpath}" >&2
	done


	# TODO: once support for rootfs images with kata-agent as init is in place,
	# uncomment the following line
#	for d in ${distrosSystemd[@]} ${distrosAgent[@]}; do
	for d in ${distrosSystemd[@]}; do
		local rootfs_path="${tmp_rootfs}/${d}_rootfs"
		local image_path="${images_dir}/kata-containers-image-$d.img"
		local rootfs_size=$(get_rootfs_size "$rootfs_path")

		echo -e "$separator"
		info "Making rootfs image for ${d}"
		make_image ${commonMakeVars[@]} $d
		local image_size=$(stat -c "%s" "${image_path}")

		echo -e "$separator"
		built_images["${d}"]="${rootfs_size}:${image_size}"
		info "Creating container for ${d}"
		install_image_create_container $image_path
	done

	for d in ${distrosAgent[@]}; do
		local rootfs_path="${tmp_rootfs}/${d}_rootfs"
		local initrd_path="${images_dir}/kata-containers-initrd-$d.img"
		local rootfs_size=$(get_rootfs_size "$rootfs_path")

		echo -e "$separator"
		info "Making initrd image for ${d}"
		make_initrd ${commonMakeVars[@]} AGENT_INIT=yes $d
		local initrd_size=$(stat -c "%s" "${initrd_path}")

		echo -e "$separator"
		built_initrds["${d}"]="${rootfs_size}:${initrd_size}"
		info "Creating container for ${d}"
		install_initrd_create_container $initrd_path
	done

	echo -e "$separator"
	show_stats

	echo -e "$separator"
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
			-h|--help) usage; exit 0 ;;

			--list) list_distros; exit 0;;

			--test-images-only)
				build_initrds=
				;;

			--test-initrds-only)
				build_images=
				;;

			--) shift; break ;;
		esac

		shift
	done

	# Consume getopt cruft
	[ "$1" = "--" ] && shift

	case "${1:-}" in
		help) usage; exit 0;;

		*) distro="${1:-}";;
	esac

	trap exit_handler EXIT ERR
	setup

	test_distros "$distro"

	# We shouldn't really need a message like this but the CI can fail in
	# mysterious ways so make it clear!
	info "all tests finished successfully"
}

main "$@"
