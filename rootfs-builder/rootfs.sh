#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

[ -n "$DEBUG" ] && set -x

script_name="${0##*/}"
script_dir="$(dirname $(readlink -f $0))"
AGENT_VERSION=${AGENT_VERSION:-}
GO_AGENT_PKG=${GO_AGENT_PKG:-github.com/kata-containers/agent}
AGENT_BIN=${AGENT_BIN:-kata-agent}
AGENT_INIT=${AGENT_INIT:-no}
KERNEL_MODULES_DIR=${KERNEL_MODULES_DIR:-""}
OSBUILDER_VERSION="unknown"

lib_file="${script_dir}/../scripts/lib.sh"
source "$lib_file"

# Default architecture
ARCH=$(arch)

# Load default versions for golang and other componets
source "${script_dir}/versions.txt"

# distro-specific config file
typeset -r CONFIG_SH="config.sh"

# optional arch-specific config file
typeset -r CONFIG_ARCH_SH="config_${ARCH}.sh"

# Name of an optional distro-specific file which, if it exists, must implement the
# build_rootfs() function.
typeset -r LIB_SH="rootfs_lib.sh"

#$1: Error code if want to exit different to 0
usage()
{
	error="${1:-0}"
	cat <<EOT

Usage: ${script_name} [options] <distro>

Build a rootfs based on <distro> OS, to be included in a Kata Containers
image.

Supported <distro> values:
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
  -t                Print the test configuration for <distro> and exit
                    immediately.

Environment Variables:
AGENT_BIN           Name of the agent binary (used when running sanity checks on
                    the rootfs).
                    Default value: ${AGENT_BIN}

AGENT_INIT          When set to "yes", use ${AGENT_BIN} as init process in place
                    of systemd.
                    Default value: no

AGENT_VERSION       Version of the agent to include in the rootfs.
                    Default value: ${AGENT_VERSION:-<not set>}

GO_AGENT_PKG        URL of the Git repository hosting the agent package.
                    Default value: ${GO_AGENT_PKG}

GRACEFUL_EXIT       If set, and if the <distro> configuration specifies a
                    non-empty BUILD_CAN_FAIL variable, do not return with an
                    error code in case any of the build step fails.
                    This is used when running CI jobs, to tolerate failures for
                    specific distributions.
                    Default value: <not set>

KERNEL_MODULES_DIR  Path to a directory containing kernel modules to include in
                    the rootfs.
                    Default value: <empty>

ROOTFS_DIR          Path to the directory that is populated with the rootfs.
                    Default value: <${script_name} path>/rootfs-<distro-name>

USE_DOCKER          If set, build the rootfs inside a container (requires
                    Docker).
                    Default value: <not set>

Refer to the Platform-OS Compatibility Matrix for more details on the supported
architectures:
https://github.com/kata-containers/osbuilder#platform-distro-compatibility-matrix

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
	local distro="$1"
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

	case "$1" in
	 ubuntu | debian)
		# Requred to chroot
		args+=" --cap-add SYS_CHROOT"
		# debootstrap needs to create device nodes to properly function
		args+=" --cap-add MKNOD"
		;&
	suse)
		# Required to mount inside a container
		args+=" --cap-add SYS_ADMIN"
		# When AppArmor is enabled, mounting inside a container is blocked with docker-default profile.
		# See https://github.com/moby/moby/issues/16429
		args+=" --security-opt apparmor:unconfined"
		;;
	*)
		;;
	esac

	echo "$args"
}

generate_dockerfile()
{
	dir="$1"

	case "$(arch)" in
		"ppc64le")
			goarch=ppc64le
			;;

		"aarch64")
			goarch=arm64
			;;

		*)
			goarch=amd64
			;;
	esac

	[ -n "$http_proxy" ] && readonly set_proxy="RUN sed -i '$ a proxy="$http_proxy"' /etc/dnf/dnf.conf /etc/yum.conf; true"

	curlOptions=("-OL")
	[ -n "$http_proxy" ] && curlOptions+=("-x $http_proxy")
	readonly install_go="
RUN cd /tmp ; curl ${curlOptions[@]} https://storage.googleapis.com/golang/go${GO_VERSION}.linux-${goarch}.tar.gz
RUN tar -C /usr/ -xzf /tmp/go${GO_VERSION}.linux-${goarch}.tar.gz
ENV GOROOT=/usr/go
ENV PATH=\$PATH:\$GOROOT/bin:\$GOPATH/bin
"

	readonly dockerfile_template="Dockerfile.in"
	[ -d "${dir}" ] || die "${dir}: not a directory"
	pushd ${dir}
	[ -f "${dockerfile_template}" ] || die "${dockerfile_template}: file not found"
	sed \
		-e "s|@GO_VERSION@|${GO_VERSION}|g" \
		-e "s|@OS_VERSION@|${OS_VERSION}|g" \
		-e "s|@INSTALL_GO@|${install_go//$'\n'/\\n}|g" \
		-e "s|@SET_PROXY@|${set_proxy}|g" \
		${dockerfile_template} > Dockerfile
	popd
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

while getopts a:hlo:r:t: opt
do
	case $opt in
		a)	AGENT_VERSION="${OPTARG}" ;;
		h)	usage ;;
		l)	get_distros | sort && exit 0;;
		o)	OSBUILDER_VERSION="${OPTARG}" ;;
		r)	ROOTFS_DIR="${OPTARG}" ;;
		t)	get_test_config "${OPTARG}" && exit 0;;
	esac
done

shift $(($OPTIND - 1))

# Fetch the first element from GOPATH as working directory
# as go get only works against the first item in the GOPATH
[ -z "$GOPATH" ] && die "GOPATH not set"
GOPATH_LOCAL="${GOPATH%%:*}"

[ "$AGENT_INIT" == "yes" -o "$AGENT_INIT" == "no" ] || die "AGENT_INIT($AGENT_INIT) is invalid (must be yes or no)"

[ -n "${KERNEL_MODULES_DIR}" ] && [ ! -d "${KERNEL_MODULES_DIR}" ] && die "KERNEL_MODULES_DIR defined but is not an existing directory"

[ -z "${OSBUILDER_VERSION}" ] && die "need osbuilder version"

distro="$1"

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

init="${ROOTFS_DIR}/sbin/init"

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

if [ -n "${USE_DOCKER}" ] ; then
	image_name="${distro}-rootfs-osbuilder"

	generate_dockerfile "${distro_config_dir}"
	docker build  \
		--build-arg http_proxy="${http_proxy}" \
		--build-arg https_proxy="${https_proxy}" \
		-t "${image_name}" "${distro_config_dir}"

	# fake mapping if KERNEL_MODULES_DIR is unset
	kernel_mod_dir=${KERNEL_MODULES_DIR:-${ROOTFS_DIR}}

	docker_run_args=""
	docker_run_args+=" --rm"
	docker_run_args+=" --runtime runc"

	docker_run_args+=" $(docker_extra_args $distro)"

	#Make sure we use a compatible runtime to build rootfs
	# In case Clear Containers Runtime is installed we dont want to hit issue:
	#https://github.com/clearcontainers/runtime/issues/828
	docker run  \
		--env https_proxy="${https_proxy}" \
		--env http_proxy="${http_proxy}" \
		--env AGENT_VERSION="${AGENT_VERSION}" \
		--env ROOTFS_DIR="/rootfs" \
		--env GO_AGENT_PKG="${GO_AGENT_PKG}" \
		--env AGENT_BIN="${AGENT_BIN}" \
		--env AGENT_INIT="${AGENT_INIT}" \
		--env GOPATH="${GOPATH_LOCAL}" \
		--env KERNEL_MODULES_DIR="${KERNEL_MODULES_DIR}" \
		--env EXTRA_PKGS="${EXTRA_PKGS}" \
		--env OSBUILDER_VERSION="${OSBUILDER_VERSION}" \
		--env INSIDE_CONTAINER=1 \
		--env SECCOMP="${SECCOMP}" \
		-v "${script_dir}":"/osbuilder" \
		-v "${ROOTFS_DIR}":"/rootfs" \
		-v "${script_dir}/../scripts":"/scripts" \
		-v "${kernel_mod_dir}":"${kernel_mod_dir}" \
		-v "${GOPATH_LOCAL}":"${GOPATH_LOCAL}" \
		$docker_run_args \
		${image_name} \
		bash /osbuilder/rootfs.sh "${distro}"

	exit $?
fi

mkdir -p ${ROOTFS_DIR}
build_rootfs ${ROOTFS_DIR}

[ -n "${KERNEL_MODULES_DIR}" ] && copy_kernel_modules ${KERNEL_MODULES_DIR} ${ROOTFS_DIR}

info "Pull Agent source code"
go get -d "${GO_AGENT_PKG}" || true
OK "Pull Agent source code"

info "Build agent"
pushd "${GOPATH_LOCAL}/src/${GO_AGENT_PKG}"
[ -n "${AGENT_VERSION}" ] && git checkout "${AGENT_VERSION}" && OK "git checkout successful"
make clean
make INIT=${AGENT_INIT}
make install DESTDIR="${ROOTFS_DIR}" INIT=${AGENT_INIT} SECCOMP=${SECCOMP}
popd

AGENT_DIR="${ROOTFS_DIR}/usr/bin"
AGENT_DEST="${AGENT_DIR}/${AGENT_BIN}"
[ -x "${AGENT_DEST}" ] || die "${AGENT_DEST} is not installed in ${ROOTFS_DIR}"
OK "Agent installed"

[ "${AGENT_INIT}" == "yes" ] && setup_agent_init "${AGENT_DEST}" "${init}"

info "Check init is installed"
[ -x "${init}" ] || [ -L "${init}" ] || die "/sbin/init is not installed in ${ROOTFS_DIR}"
OK "init is installed"

info "Creating summary file"
create_summary_file "${ROOTFS_DIR}"
