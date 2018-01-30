#!/bin/bash
#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

script_name="${0##*/}"
script_dir="$(dirname $(readlink -f $0))"
ROOTFS_DIR=${ROOTFS_DIR:-${PWD}/rootfs}
AGENT_VERSION=${AGENT_VERSION:-master}
GO_AGENT_PKG=${GO_AGENT_PKG:-github.com/kata-containers/agent}
AGENT_BIN=${AGENT_BIN:-kata-agent}
AGENT_INIT=${AGENT_INIT:-no}

#Load default vesions for golang and other componets
source "${script_dir}/versions.txt"

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
AGENT_INIT  : Use $(AGENT_BIN) as init process.
            DEFAULT: no
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

generate_dockerfile() {
	dir="$1"

	readonly install_go="
ADD https://storage.googleapis.com/golang/go${GO_VERSION}.linux-amd64.tar.gz /tmp
RUN tar -C /usr/ -xzf /tmp/go${GO_VERSION}.linux-amd64.tar.gz
ENV GOROOT=/usr/go
ENV PATH=\$PATH:\$GOROOT/bin:\$GOPATH/bin
"

	readonly dockerfile_template="Dockerfile.in"
	[ -d "${dir}" ] || die "${dir}: not a directory"
	pushd ${dir}
	[ -f "${dockerfile_template}" ] || die "${dockerfile_template}: file not found"
	sed \
		-e "s|@OS_VERSION@|${OS_VERSION}|g" \
		-e "s|@INSTALL_GO@|${install_go//$'\n'/\\n}|g" \
		${dockerfile_template} > Dockerfile
	popd
}

setup_agent_init() {
	agent_bin="$1"
	init_bin="$2"
	info "Install $agent_bin as init process"
	mv -f "${agent_bin}" ${init_bin}
	OK "Agent is installed as init process"
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

[ -z "$GOPATH" ] && die "GOPATH not set"

[ "$AGENT_INIT" == "yes" -o "$AGENT_INIT" == "no" ] || die "AGENT_INIT($AGENT_INIT) is invalid (must be yes or no)"

distro="$1"
init="${ROOTFS_DIR}/sbin/init"

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

	generate_dockerfile "${distro_config_dir}"
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
		--env AGENT_INIT="${AGENT_INIT}" \
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

info "Pull Agent source code"
go get -d "${GO_AGENT_PKG}" || true
OK "Pull Agent source code"

info "Build agent"
pushd "${GOPATH}/src/${GO_AGENT_PKG}"
make INIT=${AGENT_INIT}
make install DESTDIR="${ROOTFS_DIR}" INIT=${AGENT_INIT}
popd
[ -x "${ROOTFS_DIR}/bin/${AGENT_BIN}" ] || die "/bin/${AGENT_BIN} is not installed in ${ROOTFS_DIR}"
OK "Agent installed"

[ "${AGENT_INIT}" == "yes" ] && setup_agent_init "${ROOTFS_DIR}/bin/${AGENT_BIN}" "${init}"

info "Check init is installed"
[ -x "${init}" ] || [ -L ${init} ] || die "/sbin/init is not installed in ${ROOTFS_DIR}"
OK "init is installed"
