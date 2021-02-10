#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o pipefail
set -o errtrace

[ -n "$DEBUG" ] && set -x

script_name="${0##*/}"
script_dir="$(dirname $(readlink -f $0))"
AGENT_VERSION=${AGENT_VERSION:-}
GO_AGENT_PKG=${GO_AGENT_PKG:-github.com/kata-containers/agent}
RUST_AGENT_PKG=${RUST_AGENT_PKG:-github.com/kata-containers/kata-containers}
RUST_AGENT=${RUST_AGENT:-yes}
RUST_VERSION="null"
MUSL_VERSION=${MUSL_VERSION:-"null"}
AGENT_BIN=${AGENT_BIN:-kata-agent}
AGENT_INIT=${AGENT_INIT:-no}
KERNEL_MODULES_DIR=${KERNEL_MODULES_DIR:-""}
OSBUILDER_VERSION="unknown"
DOCKER_RUNTIME=${DOCKER_RUNTIME:-runc}
GO_VERSION="null"
export GOPATH=${GOPATH:-${HOME}/go}
LIBC=${LIBC:-musl}

lib_file="${script_dir}/../scripts/lib.sh"
source "$lib_file"

handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"

}
trap 'handle_error $LINENO' ERR

# Default architecture
ARCH=$(uname -m)

# distro-specific config file
typeset -r CONFIG_SH="config.sh"

# optional arch-specific config file
typeset -r CONFIG_ARCH_SH="config_${ARCH}.sh"

# Name of an optional distro-specific file which, if it exists, must implement the
# build_rootfs() function.
typeset -r LIB_SH="rootfs_lib.sh"

# rootfs distro name specified by the user
typeset distro=

# Absolute path to the rootfs root folder
typeset ROOTFS_DIR

# Absolute path in the rootfs to the "init" executable / symlink.
# Typically something like "${ROOTFS_DIR}/init
typeset init=

#$1: Error code if want to exit different to 0
usage()
{
	error="${1:-0}"
	cat <<EOT

Usage: ${script_name} [options] [DISTRO]

Build and setup a rootfs directory based on DISTRO OS, used to create
Kata Containers images or initramfs.

When no DISTRO is provided, an existing base rootfs at ROOTFS_DIR is provisioned
with the Kata specific components and configuration.

Supported DISTRO values:
$(get_distros | tr "\n" " ")

Options:
  -a <version>      Specify the agent version. Overrides the AGENT_VERSION
                    environment variable.
  -h                Show this help message.
  -l                List the supported Linux distributions and exit immediately.
  -o <version>      Specify the version of osbuilder to embed in the rootfs
                    yaml description.
  -r <directory>    Specify the rootfs base directory. Overrides the ROOTFS_DIR
                    environment variable.
  -t DISTRO         Print the test configuration for DISTRO and exit
                    immediately.

Environment Variables:
AGENT_BIN           Name of the agent binary (used when running sanity checks on
                    the rootfs).
                    Default value: ${AGENT_BIN}

AGENT_INIT          When set to "yes", use ${AGENT_BIN} as init process in place
                    of systemd.
                    Default value: no

AGENT_SOURCE_BIN    Path to the directory of agent binary.
                    If set, use the binary as agent but not build agent package.
                    Default value: <not set>

AGENT_VERSION       Version of the agent to include in the rootfs.
                    Default value: ${AGENT_VERSION:-<not set>}

DISTRO_REPO         Use host repositories to install guest packages.
                    Default value: <not set>

DOCKER_RUNTIME      Docker runtime to use when USE_DOCKER is set.
                    Default value: runc

GO_AGENT_PKG        URL of the Git repository hosting the agent package.
                    Default value: ${GO_AGENT_PKG}

GRACEFUL_EXIT       If set, and if the DISTRO configuration specifies a
                    non-empty BUILD_CAN_FAIL variable, do not return with an
                    error code in case any of the build step fails.
                    This is used when running CI jobs, to tolerate failures for
                    specific distributions.
                    Default value: <not set>

IMAGE_REGISTRY      Hostname for the image registry used to pull down the rootfs
                    build image.
                    Default value: docker.io

KERNEL_MODULES_DIR  Path to a directory containing kernel modules to include in
                    the rootfs.
                    Default value: <empty>

ROOTFS_DIR          Path to the directory that is populated with the rootfs.
                    Default value: <${script_name} path>/rootfs-<DISTRO-name>

RUST_AGENT          When set to "no", build kata-agent from go agent instead of kata-rust-agent
                    Default value: "yes"

RUST_AGENT_PKG      URL of the Git repository hosting the agent package.
                    Default value: ${RUST_AGENT_PKG}

USE_DOCKER          If set, build the rootfs inside a container (requires
                    Docker).
                    Default value: <not set>

USE_PODMAN          If set and USE_DOCKER not set, then build the rootfs inside
                    a podman container (requires podman).
                    Default value: <not set>

Refer to the Platform-OS Compatibility Matrix for more details on the supported
architectures:
https://github.com/kata-containers/kata-containers/tree/main/tools/osbuilder#platform-distro-compatibility-matrix

EOT
exit "${error}"
}

get_distros() {
	cdirs=$(find "${script_dir}" -maxdepth 1 -type d)
	find ${cdirs} -maxdepth 1 -name "${CONFIG_SH}" -printf '%H\n' | while read dir; do
		basename "${dir}"
	done
}

get_test_config() {
	local -r distro="$1"
	[ -z "$distro" ] && die "No distro name specified"

	local config="${script_dir}/${distro}/config.sh"
	source ${config}

	echo -e "INIT_PROCESS:\t\t$INIT_PROCESS"
	echo -e "ARCH_EXCLUDE_LIST:\t\t${ARCH_EXCLUDE_LIST[@]}"
}

check_function_exist()
{
	function_name="$1"
	[ "$(type -t ${function_name})" == "function" ] || die "${function_name} function was not defined"
}

docker_extra_args()
{
	local args=""

	# Required to mount inside a container
	args+=" --cap-add SYS_ADMIN"
	# Requred to chroot
	args+=" --cap-add SYS_CHROOT"
	# debootstrap needs to create device nodes to properly function
	args+=" --cap-add MKNOD"

	case "$1" in
	gentoo)
		# Required to build glibc
		args+=" --cap-add SYS_PTRACE"
		# mount portage volume
		args+=" -v ${gentoo_local_portage_dir}:/usr/portage/packages"
		args+=" --volumes-from ${gentoo_portage_container}"
		;;
        debian | ubuntu | suse)
		source /etc/os-release

		case "$ID" in
                fedora | centos | rhel)
			# Depending on the podman version, we'll face issues when passing
		        # `--security-opt apparmor=unconfined` on a system where not apparmor is not installed.
			# Because of this, let's just avoid adding this option when the host OS comes from Red Hat.

			# A explict check for podman, at least for now, can be avoided.
			;;
		*)
			# When AppArmor is enabled, mounting inside a container is blocked with docker-default profile.
			# See https://github.com/moby/moby/issues/16429
			args+=" --security-opt apparmor=unconfined"
			;;
		esac
		;;
	*)
		;;
	esac

	echo "$args"
}

setup_agent_init()
{
	agent_bin="$1"
	init_bin="$2"

	[ -z "$agent_bin" ] && die "need agent binary path"
	[ -z "$init_bin" ] && die "need init bin path"

	info "Install $agent_bin as init process"
	mv -f "${agent_bin}" ${init_bin}
	OK "Agent is installed as init process"
}

copy_kernel_modules()
{
	local module_dir="$1"
	local rootfs_dir="$2"

	[ -z "$module_dir" ] && die "need module directory"
	[ -z "$rootfs_dir" ] && die "need rootfs directory"

	local dest_dir="${rootfs_dir}/lib/modules"

	info "Copy kernel modules from ${KERNEL_MODULES_DIR}"
	mkdir -p "${dest_dir}"
	cp -a "${KERNEL_MODULES_DIR}" "${dest_dir}/"
	OK "Kernel modules copied"
}

error_handler()
{
	[ "$?" -eq 0 ] && return

	if [ -n "$GRACEFUL_EXIT" ] && [ -n "$BUILD_CAN_FAIL" ]; then
		info "Detected a build error, but $distro is allowed to fail (BUILD_CAN_FAIL specified), so exiting sucessfully"
		touch "$(dirname ${ROOTFS_DIR})/${distro}_fail"
		exit 0
	fi
}

# Compares two SEMVER-style versions passed as arguments, up to the MINOR version
# number.
# Returns a zero exit code if the version specified by the first argument is
# older OR equal than / to the version in the second argument, non-zero exit
# code otherwise.
compare_versions()
{
	typeset -i -a v1=($(echo "$1" | awk 'BEGIN {FS = "."} {print $1" "$2}'))
	typeset -i -a v2=($(echo "$2" | awk 'BEGIN {FS = "."} {print $1" "$2}'))

	# Sanity check: first version can't be all zero
	[ "${v1[0]}" -eq "0" ] && \
		[ "${v1[1]}" -eq "0" ] && \
		die "Failed to parse version number"

	# Major
	[ "${v1[0]}" -gt "${v2[0]}" ] && { false; return; }

	# Minor
	[ "${v1[0]}" -eq "${v2[0]}" ] && \
		[ "${v1[1]}" -gt "${v2[1]}" ] && { false; return; }

	true
}

check_env_variables()
{
	# Fetch the first element from GOPATH as working directory
	# as go get only works against the first item in the GOPATH
	[ -z "$GOPATH" ] && die "GOPATH not set"
	GOPATH_LOCAL="${GOPATH%%:*}"

	[ "$AGENT_INIT" == "yes" -o "$AGENT_INIT" == "no" ] || die "AGENT_INIT($AGENT_INIT) is invalid (must be yes or no)"

	if [ -z "${AGENT_SOURCE_BIN}" ]; then
		[ "$RUST_AGENT" == "yes" -o "$RUST_AGENT" == "no" ] || die "RUST_AGENT($RUST_AGENT) is invalid (must be yes or no)"
	fi

	[ -n "${KERNEL_MODULES_DIR}" ] && [ ! -d "${KERNEL_MODULES_DIR}" ] && die "KERNEL_MODULES_DIR defined but is not an existing directory"

	[ -n "${OSBUILDER_VERSION}" ] || die "need osbuilder version"
}

# Builds a rootfs based on the distro name provided as argument
build_rootfs_distro()
{
	repo_dir="${script_dir}/../../../"
	[ -n "${distro}" ] || usage 1
	distro_config_dir="${script_dir}/${distro}"

	# Source config.sh from distro
	rootfs_config="${distro_config_dir}/${CONFIG_SH}"
	source "${rootfs_config}"

	# Source arch-specific config file
	rootfs_arch_config="${distro_config_dir}/${CONFIG_ARCH_SH}"
	if [ -f "${rootfs_arch_config}" ]; then
		source "${rootfs_arch_config}"
	fi

	[ -d "${distro_config_dir}" ] || die "Not found configuration directory ${distro_config_dir}"

	if [ -z "$ROOTFS_DIR" ]; then
		 ROOTFS_DIR="${script_dir}/rootfs-${OS_NAME}"
	fi

	if [ -e "${distro_config_dir}/${LIB_SH}" ];then
		rootfs_lib="${distro_config_dir}/${LIB_SH}"
		info "rootfs_lib.sh file found. Loading content"
		source "${rootfs_lib}"
	fi

	CONFIG_DIR=${distro_config_dir}
	check_function_exist "build_rootfs"

	if [ -z "$INSIDE_CONTAINER" ] ; then
		# Capture errors, but only outside of the docker container
		trap error_handler ERR
	fi

	mkdir -p ${ROOTFS_DIR}

	detect_go_version ||
		die "Could not detect the required Go version for AGENT_VERSION='${AGENT_VERSION:-master}'."

	echo "Required Go version: $GO_VERSION"

	# need to detect rustc's version too?
	detect_rust_version ||
		die "Could not detect the required rust version for AGENT_VERSION='${AGENT_VERSION:-master}'."

	echo "Required rust version: $RUST_VERSION"

	detect_musl_version ||
		die "Could not detect the required musl version for AGENT_VERSION='${AGENT_VERSION:-master}'."

	echo "Required musl version: $MUSL_VERSION"

	if [ -z "${USE_DOCKER}" ] && [ -z "${USE_PODMAN}" ]; then
		#Generate an error if the local Go version is too old
		info "build directly"
		build_rootfs ${ROOTFS_DIR}
	else
		if [ -n "${USE_DOCKER}" ]; then
			container_engine="docker"
		elif [ -n "${USE_PODMAN}" ]; then
			container_engine="podman"
		fi

		image_name="${distro}-rootfs-osbuilder"

		REGISTRY_ARG=""
		if [ -n "${IMAGE_REGISTRY}" ]; then
			REGISTRY_ARG="--build-arg IMAGE_REGISTRY=${IMAGE_REGISTRY}"
		fi

		# setup to install go or rust here
		generate_dockerfile "${distro_config_dir}"
		"$container_engine" build  \
			${REGISTRY_ARG} \
			--build-arg http_proxy="${http_proxy}" \
			--build-arg https_proxy="${https_proxy}" \
			-t "${image_name}" "${distro_config_dir}"

		# fake mapping if KERNEL_MODULES_DIR is unset
		kernel_mod_dir=${KERNEL_MODULES_DIR:-${ROOTFS_DIR}}

		docker_run_args=""
		docker_run_args+=" --rm"
		docker_run_args+=" --runtime ${DOCKER_RUNTIME}"

		if [ -z "${AGENT_SOURCE_BIN}" ] ; then
			if [ "$RUST_AGENT" == "no" ]; then
				docker_run_args+=" --env GO_AGENT_PKG=${GO_AGENT_PKG}"
			else
				docker_run_args+=" --env RUST_AGENT_PKG=${RUST_AGENT_PKG}"
			fi
			docker_run_args+=" --env RUST_AGENT=${RUST_AGENT} -v ${GOPATH_LOCAL}:${GOPATH_LOCAL} --env GOPATH=${GOPATH_LOCAL}"
		else
			docker_run_args+=" --env AGENT_SOURCE_BIN=${AGENT_SOURCE_BIN}"
			docker_run_args+=" -v ${AGENT_SOURCE_BIN}:${AGENT_SOURCE_BIN}"
			docker_run_args+=" -v ${GOPATH_LOCAL}:${GOPATH_LOCAL} --env GOPATH=${GOPATH_LOCAL}"
		fi

		docker_run_args+=" $(docker_extra_args $distro)"

		# Relabel volumes so SELinux allows access (see docker-run(1))
		if command -v selinuxenabled > /dev/null && selinuxenabled ; then
			SRC_VOL=("${GOPATH_LOCAL}")

			for volume_dir in "${script_dir}" \
					  "${ROOTFS_DIR}" \
					  "${script_dir}/../scripts" \
					  "${kernel_mod_dir}" \
					  "${SRC_VOL[@]}"; do
				chcon -Rt svirt_sandbox_file_t "$volume_dir"
			done
		fi

		before_starting_container
		trap after_stopping_container EXIT

		#Make sure we use a compatible runtime to build rootfs
		# In case Clear Containers Runtime is installed we dont want to hit issue:
		#https://github.com/clearcontainers/runtime/issues/828
		"$container_engine" run  \
			--env https_proxy="${https_proxy}" \
			--env http_proxy="${http_proxy}" \
			--env AGENT_VERSION="${AGENT_VERSION}" \
			--env ROOTFS_DIR="/rootfs" \
			--env AGENT_BIN="${AGENT_BIN}" \
			--env AGENT_INIT="${AGENT_INIT}" \
			--env KERNEL_MODULES_DIR="${KERNEL_MODULES_DIR}" \
			--env EXTRA_PKGS="${EXTRA_PKGS}" \
			--env OSBUILDER_VERSION="${OSBUILDER_VERSION}" \
			--env INSIDE_CONTAINER=1 \
			--env SECCOMP="${SECCOMP}" \
			--env DEBUG="${DEBUG}" \
			--env STAGE_PREPARE_ROOTFS=1 \
			--env HOME="/root" \
			-v "${repo_dir}":"/kata-containers" \
			-v "${ROOTFS_DIR}":"/rootfs" \
			-v "${script_dir}/../scripts":"/scripts" \
			-v "${kernel_mod_dir}":"${kernel_mod_dir}" \
			$docker_run_args \
			${image_name} \
			bash /kata-containers/tools/osbuilder/rootfs-builder/rootfs.sh "${distro}"
	fi
}

# Used to create a minimal directory tree where the agent can be instaleld.
# This is used when a distro is not specified.
prepare_overlay()
{
	pushd "${ROOTFS_DIR}" > /dev/null
	mkdir -p ./etc ./lib/systemd ./sbin ./var

	# This symlink hacking is mostly to make later rootfs
	# validation work correctly for the dracut case.
	# We skip this if /sbin/init exists in the rootfs, meaning
	# we were passed a pre-populated rootfs directory
	if [ ! -e ./sbin/init ]; then
		ln -sf  ./usr/lib/systemd/systemd ./init
		ln -sf  /init ./sbin/init
	fi

	# Kata systemd unit file
	mkdir -p ./etc/systemd/system/basic.target.wants/
	ln -sf /usr/lib/systemd/system/kata-containers.target ./etc/systemd/system/basic.target.wants/kata-containers.target
	popd  > /dev/null
}

# Setup an existing rootfs directory, based on the OPTIONAL distro name
# provided as argument
setup_rootfs()
{
	info "Create symlink to /tmp in /var to create private temporal directories with systemd"
	pushd "${ROOTFS_DIR}" >> /dev/null
	if [ "$PWD" != "/" ] ; then
		rm -rf ./var/cache/ ./var/lib ./var/log ./var/tmp
	fi

	ln -s ../tmp ./var/

	# For some distros tmp.mount may not be installed by default in systemd paths
	if ! [ -f "./etc/systemd/system/tmp.mount" ] && \
		! [ -f "./usr/lib/systemd/system/tmp.mount" ] &&
		[ "$AGENT_INIT" != "yes" ]; then
		local unitFile="./etc/systemd/system/tmp.mount"
		info "Install tmp.mount in ./etc/systemd/system"
		mkdir -p `dirname "$unitFile"`
		cp ./usr/share/systemd/tmp.mount "$unitFile" || cat > "$unitFile" << EOT
#  This file is part of systemd.
#
#  systemd is free software; you can redistribute it and/or modify it
#  under the terms of the GNU Lesser General Public License as published by
#  the Free Software Foundation; either version 2.1 of the License, or
#  (at your option) any later version.

[Unit]
Description=Temporary Directory (/tmp)
Documentation=man:hier(7)
Documentation=https://www.freedesktop.org/wiki/Software/systemd/APIFileSystems
ConditionPathIsSymbolicLink=!/tmp
DefaultDependencies=no
Conflicts=umount.target
Before=local-fs.target umount.target
After=swap.target

[Mount]
What=tmpfs
Where=/tmp
Type=tmpfs
Options=mode=1777,strictatime,nosuid,nodev
EOT
	fi

	popd  >> /dev/null

	[ -n "${KERNEL_MODULES_DIR}" ] && copy_kernel_modules ${KERNEL_MODULES_DIR} ${ROOTFS_DIR}

	info "Create ${ROOTFS_DIR}/etc"
	mkdir -p "${ROOTFS_DIR}/etc"

	case "${distro}" in
		"gentoo")
			chrony_conf_file="${ROOTFS_DIR}/etc/chrony/chrony.conf"
			chrony_systemd_service="${ROOTFS_DIR}/lib/systemd/system/chronyd.service"
			;;
		"ubuntu" | "debian")
			echo "I am ubuntu or debian"
			chrony_conf_file="${ROOTFS_DIR}/etc/chrony/chrony.conf"
			chrony_systemd_service="${ROOTFS_DIR}/lib/systemd/system/chrony.service"
			;;
		*)
			chrony_conf_file="${ROOTFS_DIR}/etc/chrony.conf"
			chrony_systemd_service="${ROOTFS_DIR}/usr/lib/systemd/system/chronyd.service"
			;;
	esac

	info "Configure chrony file ${chrony_conf_file}"
	cat >> "${chrony_conf_file}" <<EOT
refclock PHC /dev/ptp0 poll 3 dpoll -2 offset 0
# Step the system clock instead of slewing it if the adjustment is larger than
# one second, at any time
makestep 1 -1
EOT

	# Comment out ntp sources for chrony to be extra careful
	# Reference:  https://chrony.tuxfamily.org/doc/3.4/chrony.conf.html
	sed -i 's/^\(server \|pool \|peer \)/# &/g'  ${chrony_conf_file}

	if [ -f "$chrony_systemd_service" ]; then
		# Remove user option, user could not exist in the rootfs
		sed -i -e 's/^\(ExecStart=.*\)-u [[:alnum:]]*/\1/g' \
		       -e '/^\[Unit\]/a ConditionPathExists=\/dev\/ptp0' ${chrony_systemd_service}
	fi

	# The CC on s390x for fedora needs to be manually set to gcc when the golang is downloaded from the main page.
	# See issue: https://github.com/kata-containers/osbuilder/issues/217
	[ "$distro" == "fedora" ] && [ "$ARCH" == "s390x" ] && export CC=gcc

	AGENT_DIR="${ROOTFS_DIR}/usr/bin"
	AGENT_DEST="${AGENT_DIR}/${AGENT_BIN}"

	if [ -z "${AGENT_SOURCE_BIN}" ] ; then
		[ "$ARCH" == "ppc64le" ] && { LIBC=gnu; echo "WARNING: Forcing LIBC=gnu for ppc64le because musl toolchain is not supported on ppc64le"; }
		[ "$LIBC" == "musl" ] && bash ${script_dir}/../../../ci/install_musl.sh
		# rust agent needs ${arch}-unknown-linux-${LIBC}
		rustup show | grep linux-${LIBC} > /dev/null || bash ${script_dir}/../../../ci/install_rust.sh
		test -r "${HOME}/.cargo/env" && source "${HOME}/.cargo/env"
		[ "$ARCH" == "aarch64" ] && OLD_PATH=$PATH && export PATH=$PATH:/usr/local/musl/bin

		agent_pkg="${RUST_AGENT_PKG}"
		agent_dir="${script_dir}/../../../src/agent/"
		# For now, rust-agent doesn't support seccomp yet.
		SECCOMP="no"

		info "Build agent"
		pushd "${agent_dir}"
		[ -n "${AGENT_VERSION}" ] && git checkout "${AGENT_VERSION}" && OK "git checkout successful" || info "checkout failed!"
		make clean
		make LIBC=${LIBC} INIT=${AGENT_INIT}
		make install DESTDIR="${ROOTFS_DIR}" LIBC=${LIBC} INIT=${AGENT_INIT} SECCOMP=${SECCOMP}
		[ "$ARCH" == "aarch64" ] && export PATH=$OLD_PATH && rm -rf /usr/local/musl
		popd
	else
		mkdir -p ${AGENT_DIR}
		cp ${AGENT_SOURCE_BIN} ${AGENT_DEST}
		OK "cp ${AGENT_SOURCE_BIN} ${AGENT_DEST}"
	fi

	[ -x "${AGENT_DEST}" ] || die "${AGENT_DEST} is not installed in ${ROOTFS_DIR}"
	OK "Agent installed"

	[ "${AGENT_INIT}" == "yes" ] && setup_agent_init "${AGENT_DEST}" "${init}"

	info "Check init is installed"
	[ -x "${init}" ] || [ -L "${init}" ] || die "/sbin/init is not installed in ${ROOTFS_DIR}"
	OK "init is installed"

	# Create an empty /etc/resolv.conf, to allow agent to bind mount container resolv.conf to Kata VM
	dns_file="${ROOTFS_DIR}/etc/resolv.conf"
	if [ -L "$dns_file" ]; then
		# if /etc/resolv.conf is a link, it cannot be used for bind mount
		rm -f "$dns_file"
	fi
	info "Create /etc/resolv.conf file in rootfs if not exist"
	touch "$dns_file"

	info "Creating summary file"
	create_summary_file "${ROOTFS_DIR}"
}

parse_arguments()
{
	while getopts a:hlo:r:t: opt
	do
		case $opt in
			a)	AGENT_VERSION="${OPTARG}" ;;
			h)	usage ;;
			l)	get_distros | sort && exit 0;;
			o)	OSBUILDER_VERSION="${OPTARG}" ;;
			r)	ROOTFS_DIR="${OPTARG}" ;;
			t)	get_test_config "${OPTARG}" && exit 0;;
			*)  die "Found an invalid option";;
		esac
	done

	shift $(($OPTIND - 1))
	distro="$1"
	arch=$(uname -m)
}

detect_host_distro()
{
	source /etc/os-release

	case "$ID" in
		"*suse*")
			distro="suse"
			;;
		"clear-linux-os")
			distro="clearlinux"
			;;
		*)
			distro="$ID"
			;;
	esac
}

main()
{
	parse_arguments $*
	check_env_variables

	if [ -n "$distro" ]; then
		build_rootfs_distro
	else
		#Make sure ROOTFS_DIR is set correctly
		[ -d "${ROOTFS_DIR}" ] || die "Invalid rootfs directory: '$ROOTFS_DIR'"

		# Set the distro for dracut build method
		detect_host_distro
		prepare_overlay
	fi

	if [ "$STAGE_PREPARE_ROOTFS" == "" ]; then
		init="${ROOTFS_DIR}/sbin/init"
		setup_rootfs
	fi
}

main $*
