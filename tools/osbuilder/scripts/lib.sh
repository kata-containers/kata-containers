#!/usr/bin/env bash
#
# Copyright (c) 2018-2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

KATA_REPO=${KATA_REPO:-github.com/kata-containers/kata-containers}
# Give preference to variable set by CI
yq_file="${script_dir}/../../../ci/install_yq.sh"
kata_versions_file="${script_dir}/../../../versions.yaml"

error()
{
	local msg="$*"
	echo "ERROR: ${msg}" >&2
}

die()
{
	error "$*"
	exit 1
}

OK()
{
	local msg="$*"
	echo "[OK] ${msg}" >&2
}

info()
{
	local msg="$*"
	echo "INFO: ${msg}"
}

warning()
{
	local msg="$*"
	echo "WARNING: ${msg}"
}

check_program()
{
	type "$1" >/dev/null 2>&1
}

check_root()
{
	if [ "$(id -u)" != "0" ]; then
		echo "Root is needed"
		exit 1
	fi
}

generate_dnf_config()
{
	cat > "${DNF_CONF}" << EOF
[main]
reposdir=/root/mash

[base]
name=${OS_NAME}-${OS_VERSION} base
releasever=${OS_VERSION}
EOF
	if [ "$BASE_URL" != "" ]; then
		echo "baseurl=$BASE_URL" >> "$DNF_CONF"
	elif [ "$METALINK" != "" ]; then
		echo "metalink=$METALINK" >> "$DNF_CONF"
	fi

	if [ -n "$GPG_KEY_URL" ]; then
		if [ ! -f "${CONFIG_DIR}/${GPG_KEY_FILE}" ]; then
			curl -L "${GPG_KEY_URL}" -o "${CONFIG_DIR}/${GPG_KEY_FILE}"
		fi
		cat >> "${DNF_CONF}" << EOF
gpgcheck=1
gpgkey=file://${CONFIG_DIR}/${GPG_KEY_FILE}
EOF
	fi
	if [ "$SELINUX" == "yes" ]; then
		cat > "${DNF_CONF}" << EOF
[appstream]
name=${OS_NAME}-${OS_VERSION} upstream
releasever=${OS_VERSION}
EOF
		echo "metalink=$METALINK_APPSTREAM" >> "$DNF_CONF"
		if [ -n "$GPG_KEY_URL" ]; then
			if [ ! -f "${CONFIG_DIR}/${GPG_KEY_FILE}" ]; then
				curl -L "${GPG_KEY_URL}" -o "${CONFIG_DIR}/${GPG_KEY_FILE}"
			fi
			cat >> "${DNF_CONF}" << EOF
gpgcheck=1
gpgkey=file://${CONFIG_DIR}/${GPG_KEY_FILE}
EOF
		fi
	fi
}

build_rootfs()
{
	# Mandatory
	local ROOTFS_DIR="$1"

	[ -z "$ROOTFS_DIR" ] && die "need rootfs"

	# In case of support EXTRA packages, use it to allow
	# users add more packages to the base rootfs
	local EXTRA_PKGS=${EXTRA_PKGS:-""}

	#PATH where files this script is placed
	#Use it to refer to files in the same directory
	#Exmaple: ${CONFIG_DIR}/foo
	#local CONFIG_DIR=${CONFIG_DIR}

	check_root
	if [ ! -f "${DNF_CONF}" ] && [ -z "${DISTRO_REPO}" ] ; then
		DNF_CONF="./kata-${OS_NAME}-dnf.conf"
		generate_dnf_config
	fi
	mkdir -p "${ROOTFS_DIR}"
	if [ -n "${PKG_MANAGER}" ]; then
		info "DNF path provided by user: ${PKG_MANAGER}"
	elif check_program "dnf"; then
		PKG_MANAGER="dnf"
	elif check_program "yum" ; then
		PKG_MANAGER="yum"
	else
		die "neither yum nor dnf is installed"
	fi

	DNF="${PKG_MANAGER} -y --installroot=${ROOTFS_DIR} --noplugins"
	if [ -n "${DNF_CONF}" ] ; then
		DNF="${DNF} --config=${DNF_CONF}"
	else
		DNF="${DNF} --releasever=${OS_VERSION}"
	fi

	info "install packages for rootfs"
	$DNF install ${EXTRA_PKGS} ${PACKAGES}

	rm -rf ${ROOTFS_DIR}/usr/share/{bash-completion,cracklib,doc,info,locale,man,misc,pixmaps,terminfo,zoneinfo,zsh}
}

# Create a YAML metadata file inside the rootfs.
#
# This provides useful information about the rootfs than can be interrogated
# once the rootfs has been converted into a image/initrd.
create_summary_file()
{
	local -r rootfs_dir="$1"

	[ -z "$rootfs_dir" ] && die "need rootfs"

	local -r file_dir="/var/lib/osbuilder"
	local -r dir="${rootfs_dir}${file_dir}"

	local -r filename="osbuilder.yaml"
	local file="${dir}/${filename}"

	local -r now=$(date -u -d@${SOURCE_DATE_EPOCH:-$(date +%s.%N)} '+%Y-%m-%dT%T.%N%zZ')

	# sanitise package lists
	PACKAGES=$(echo "$PACKAGES"|tr ' ' '\n'|sort -u|tr '\n' ' ')
	EXTRA_PKGS=$(echo "$EXTRA_PKGS"|tr ' ' '\n'|sort -u|tr '\n' ' ')

	local -r packages=$(for pkg in ${PACKAGES}; do echo "      - \"${pkg}\""; done)
	local -r extra=$(for pkg in ${EXTRA_PKGS}; do echo "      - \"${pkg}\""; done)

	mkdir -p "$dir"

	# Semantic version of the summary file format.
	#
	# XXX: Increment every time the format of the summary file changes!
	local -r format_version="0.0.2"

	local -r osbuilder_url="https://github.com/kata-containers/kata-containers/tools/osbuilder"

	local agent="${AGENT_DEST}"
	[ "$AGENT_INIT" = yes ] && agent="${init}"

	local -r agentdir="${script_dir}/../../../"
	local agent_version=$(cat ${agentdir}/VERSION 2> /dev/null)
	[ -z "$agent_version" ] && agent_version="unknown"

	cat >"$file"<<-EOF
	---
	osbuilder:
	  url: "${osbuilder_url}"
	  version: "${OSBUILDER_VERSION}"
	rootfs-creation-time: "${now}"
	description: "osbuilder rootfs"
	file-format-version: "${format_version}"
	architecture: "${ARCH}"
	base-distro:
	  name: "${OS_NAME}"
	  version: "${OS_VERSION}"
	  packages:
	    default:
${packages}
	    extra:
${extra}
	agent:
	  url: "https://${KATA_REPO}"
	  name: "${AGENT_BIN}"
	  version: "${agent_version}"
	  agent-is-init-daemon: "${AGENT_INIT}"
EOF

	local rootfs_file="${file_dir}/$(basename "${file}")"
	info "Created summary file '${rootfs_file}' inside rootfs"
}

# generate_dockerfile takes as only argument a path. It expects a Dockerfile.in
# Dockerfile template to be present in that path, and will generate a usable
# Dockerfile replacing the '@PLACEHOLDER@' in that Dockerfile
generate_dockerfile()
{
	dir="$1"
	[ -d "${dir}" ] || die "${dir}: not a directory"

	local rustarch="$ARCH"
	[ "$ARCH" = ppc64le ] && rustarch=powerpc64le

	[ -n "${http_proxy:-}" ] && readonly set_proxy="RUN sed -i '$ a proxy="${http_proxy:-}"' /etc/dnf/dnf.conf /etc/yum.conf; true"

	# Only install Rust if agent needs to be built
	local install_rust=""

	if [ ! -z "${AGENT_SOURCE_BIN}" ] ; then
		if [ "$RUST_VERSION" == "null" ]; then
			detect_rust_version || \
				die "Could not detect the required rust version for AGENT_VERSION='${AGENT_VERSION:-main}'."
		fi
		install_rust="
ENV http_proxy=${http_proxy:-}
ENV https_proxy=${http_proxy:-}
RUN curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSLf | \
    sh -s -- -y --default-toolchain ${RUST_VERSION} -t ${rustarch}-unknown-linux-${LIBC}
RUN . /root/.cargo/env; cargo install cargo-when
"
	fi

	pushd "${dir}"

	sed \
		-e "s#@OS_VERSION@#${OS_VERSION:-}#g" \
		-e "s#@ARCH@#$ARCH#g" \
		-e "s#@INSTALL_RUST@#${install_rust//$'\n'/\\n}#g" \
		-e "s#@SET_PROXY@#${set_proxy:-}#g" \
		Dockerfile.in > Dockerfile
	popd
}

get_package_version_from_kata_yaml()
{
    local yq_path="$1"
    local yq_version
    local yq_args

	typeset -r yq=$(command -v yq || command -v "${GOPATH}/bin/yq" || echo "${GOPATH}/bin/yq")
	if [ ! -f "$yq" ]; then
		source "$yq_file"
	fi

    yq_version=$($yq -V)
    case $yq_version in
    *"version "[1-3]*)
        yq_args="r -X - ${yq_path}"
        ;;
    *)
        yq_args="e .${yq_path} -"
        ;;
    esac

	PKG_VERSION="$(cat "${kata_versions_file}" | $yq ${yq_args})"

	[ "$?" == "0" ] && [ "$PKG_VERSION" != "null" ] && echo "$PKG_VERSION" || echo ""
}

detect_rust_version()
{
	info "Detecting agent rust version"
    local yq_path="languages.rust.meta.newest-version"

	info "Get rust version from ${kata_versions_file}"
	RUST_VERSION="$(get_package_version_from_kata_yaml "$yq_path")"

	[ -n "$RUST_VERSION" ]
}

detect_libseccomp_info()
{
	info "Detecting libseccomp version"

	info "Get libseccomp version and url from ${kata_versions_file}"
	local libseccomp_ver_yq_path="externals.libseccomp.version"
	local libseccomp_url_yq_path="externals.libseccomp.url"
	export LIBSECCOMP_VERSION="$(get_package_version_from_kata_yaml "$libseccomp_ver_yq_path")"
	export LIBSECCOMP_URL="$(get_package_version_from_kata_yaml "$libseccomp_url_yq_path")"

	info "Get gperf version and url from ${kata_versions_file}"
	local gperf_ver_yq_path="externals.gperf.version"
	local gperf_url_yq_path="externals.gperf.url"
	export GPERF_VERSION="$(get_package_version_from_kata_yaml "$gperf_ver_yq_path")"
	export GPERF_URL="$(get_package_version_from_kata_yaml "$gperf_url_yq_path")"

	[ -n "$LIBSECCOMP_VERSION" ] && [ -n $GPERF_VERSION ] && [ -n "$LIBSECCOMP_URL" ] && [ -n $GPERF_URL ]
}

before_starting_container() {
	return 0
}

after_stopping_container() {
	return 0
}
