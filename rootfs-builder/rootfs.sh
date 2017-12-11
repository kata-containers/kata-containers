#!/bin/bash
#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

script_name="${0##*/}"
script_dir="$(dirname $(realpath -s $0))"
ROOTFS_DIR=${ROOTFS_DIR:-${PWD}/rootfs}
AGENT_VERSION=${AGENT_VERSION:-master}
GO_AGENT_PKG=${GO_AGENT_PKG:-github.com/kata-containers/agent}
AGENT_BIN=${AGENT_BIN:-kata-agent}
# Name of file that will implement build_rootfs
typeset -r LIB_SH="rootfs_lib.sh"

if [ -n "$DEBUG" ] ; then
	set -x
fi

#$1: Error code if want to exit differnt to 0
usage(){
	error="${1:-0}"
	cat <<EOT
USAGE: Build a Guest OS rootfs for Kata Containers image
${script_name} [options] <distro_name>

<distro_name> : Linux distribution to use as base OS.

Supported Linux distributions:

$(get_distros)

Options:
-a  : agent version DEFAULT: ${AGENT_VERSION} ENV: AGENT_VERSION 
-h  : Show this help message
-r  : rootfs directory DEFAULT: ${ROOTFS_DIR} ENV: ROOTFS_DIR

ENV VARIABLES:
GO_AGENT_PKG: Change the golang package url to get the agent source code
            DEFAULT: ${GO_AGENT_PKG}
AGENT_BIN   : Name of the agent binary (needed to check if agent is installed)
USE_DOCKER: If set will build rootfs in a Docker Container (requries docker)
            DEFAULT: not set
EOT
exit "${error}"
}

die()
{
	msg="$*"
	echo "ERROR: ${msg}" >&2
	exit 1
}

info()
{
	msg="$*"
	echo "INFO: ${msg}" >&2
}

OK()
{
	msg="$*"
	echo "INFO: [OK] ${msg}" >&2
}

get_distros() {
	cdirs=$(find "${script_dir}" -maxdepth 1 -type d)
	find ${cdirs} -maxdepth 1 -name "${LIB_SH}" -printf '%H\n' | while read dir; do
		basename "${dir}"
	done
}


check_function_exist() {
	function_name="$1"
	[ "$(type -t ${function_name})" == "function" ] || die "${function_name} function was not defined"
}


while getopts c:hr: opt
do
	case $opt in
		a)	AGENT_VERSION="${OPTARG}" ;;
		h)	usage ;;
		r)	ROOTFS_DIR="${OPTARG}" ;;
	esac
done

shift $(($OPTIND - 1))

distro="$1"

[ -n "${distro}" ] || usage 1
distro_config_dir="${script_dir}/${distro}"

[ -d "${distro_config_dir}" ] || die "Not found configuration directory ${distro_config_dir}"
rootfs_lib="${distro_config_dir}/${LIB_SH}"
source "${rootfs_lib}"
rootfs_config="${distro_config_dir}/config.sh"
source "${rootfs_config}"

CONFIG_DIR=${distro_config_dir}
check_function_exist "build_rootfs"

if [ -n "${USE_DOCKER}" ] ; then
	image_name="${distro}-rootfs-osbuilder"

	docker build  \
		--build-arg http_proxy="${http_proxy}" \
		--build-arg https_proxy="${https_proxy}" \
		-t "${image_name}" "${distro_config_dir}"

	#Make sure we use a compatible runtime to build rootfs
	# In case Clear Containers Runtime is installed we dont want to hit issue:
	#https://github.com/clearcontainers/runtime/issues/828
	docker run  \
		--runtime runc  \
		--env https_proxy="${https_proxy}" \
		--env http_proxy="${http_proxy}" \
		--env AGENT_VERSION="${AGENT_VERSION}" \
		--env ROOTFS_DIR="/rootfs" \
		--env GO_AGENT_PKG="${GO_AGENT_PKG}" \
		--env AGENT_BIN="${AGENT_BIN}" \
		--env GOPATH="${GOPATH}" \
		-v "${script_dir}":"/osbuilder" \
		-v "${ROOTFS_DIR}":"/rootfs" \
		-v "${GOPATH}":"${GOPATH}" \
		${image_name} \
		bash /osbuilder/rootfs.sh "${distro}"

	exit $?
fi

mkdir -p ${ROOTFS_DIR}
build_rootfs ${ROOTFS_DIR}

info "Check init is installed"
init="${ROOTFS_DIR}/sbin/init"
[ -x "${init}" ] || [ -L ${init} ] || die "/sbin/init is not installed in ${ROOTFS_DIR}"
OK "init is installed"

info "Pull Agent source code"
go get -d "${GO_AGENT_PKG}" || true
OK "Pull Agent source code"

info "Build agent"
pushd "${GOPATH}/src/${GO_AGENT_PKG}"
make INIT=no
make install DESTDIR="${ROOTFS_DIR}" INIT=no
popd
[ -x "${ROOTFS_DIR}/bin/${AGENT_BIN}" ] || die "/bin/${AGENT_BIN} is not installed in ${ROOTFS_DIR}"
OK "Agent installed"
