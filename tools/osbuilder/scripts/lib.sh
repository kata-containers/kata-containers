#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

GO_AGENT_PKG=${GO_AGENT_PKG:-github.com/kata-containers/agent}
GO_RUNTIME_PKG=${GO_RUNTIME_PKG:-github.com/kata-containers/runtime}
RUST_AGENT_PKG=${RUST_AGENT_PKG:-github.com/kata-containers/kata-rust-agent}
CMAKE_VERSION=${CMAKE_VERSION:-"null"}
MUSL_VERSION=${MUSL_VERSION:-"null"}
#https://github.com/kata-containers/tests/blob/master/.ci/jenkins_job_build.sh
# Give preference to variable set by CI
KATA_BRANCH=${branch:-}
KATA_BRANCH=${KATA_BRANCH:-master}
yq_file="${script_dir}/../scripts/install-yq.sh"

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
	REPO_NAME=${REPO_NAME:-"base"}
	CACHE_DIR=${CACHE_DIR:-"/var/cache/dnf"}
	cat > "${DNF_CONF}" << EOF
[main]
cachedir=${CACHE_DIR}
logfile=${LOG_FILE}
keepcache=0
debuglevel=2
exactarch=1
obsoletes=1
plugins=0
installonly_limit=3
reposdir=/root/mash
retries=5
EOF
	if [ "$BASE_URL" != "" ]; then
		cat >> "${DNF_CONF}" << EOF

[base]
name=${OS_NAME}-${OS_VERSION} ${REPO_NAME}
failovermethod=priority
baseurl=${BASE_URL}
enabled=1
EOF
	elif [ "$MIRROR_LIST" != "" ]; then
		cat >> "${DNF_CONF}" << EOF

[base]
name=${OS_NAME}-${OS_VERSION} ${REPO_NAME}
mirrorlist=${MIRROR_LIST}
enabled=1
EOF
	fi

	if [ -n "$GPG_KEY_URL" ]; then
		if [ ! -f "${CONFIG_DIR}/${GPG_KEY_FILE}" ]; then
			curl -L ${GPG_KEY_URL} -o ${CONFIG_DIR}/${GPG_KEY_FILE}
		fi
		cat >> "${DNF_CONF}" << EOF
gpgcheck=1
gpgkey=file://${CONFIG_DIR}/${GPG_KEY_FILE}
EOF
	fi

	if [ -n "$GPG_KEY_ARCH_URL" ]; then
		if [ ! -f "${CONFIG_DIR}/${GPG_KEY_ARCH_FILE}" ]; then
			 curl -L ${GPG_KEY_ARCH_URL} -o ${CONFIG_DIR}/${GPG_KEY_ARCH_FILE}
		fi
		cat >> "${DNF_CONF}" << EOF
       file://${CONFIG_DIR}/${GPG_KEY_ARCH_FILE}
EOF
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
	$DNF install ${EXTRA_PKGS} ${PACKAGES}
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

	local -r osbuilder_url="https://github.com/kata-containers/osbuilder"

	local agent="${AGENT_DEST}"
	[ "$AGENT_INIT" = yes ] && agent="${init}"

	local agent_version
	if [ "${RUST_AGENT}" == "no" ]; then
		agent_version=$("$agent" --version|awk '{print $NF}')
	else
		local -r agentdir="${GOPATH}/src/${RUST_AGENT_PKG}/src/agent"
		agent_version=$(cat ${agentdir}/VERSION)
	fi

	local REAL_AGENT_PKG

	if [ "$RUST_AGENT" == "no" ]; then
		REAL_AGENT_PKG=${GO_AGENT_PKG}
	else
		REAL_AGENT_PKG=${RUST_AGENT_PKG}
	fi

	cat >"$file"<<-EOT
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
	  url: "https://${REAL_AGENT_PKG}"
	  name: "${AGENT_BIN}"
	  version: "${agent_version}"
	  agent-is-init-daemon: "${AGENT_INIT}"
EOT

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

	local architecture=$(uname -m)
	local rustarch=${architecture}
	local muslarch=${architecture}
	case "$(uname -m)" in
		"ppc64le")
			goarch=ppc64le
			rustarch=powerpc64le
			muslarch=powerpc64
			;;

		"aarch64")
			goarch=arm64
			;;
		"s390x")
			goarch=s390x
			;;

		*)
			goarch=amd64
			;;
	esac

	[ -n "${http_proxy:-}" ] && readonly set_proxy="RUN sed -i '$ a proxy="${http_proxy:-}"' /etc/dnf/dnf.conf /etc/yum.conf; true"

	curlOptions=("-OL")
	[ -n "${http_proxy:-}" ] && curlOptions+=("-x ${http_proxy:-}")

	readonly install_go="
RUN cd /tmp ; curl ${curlOptions[@]} https://storage.googleapis.com/golang/go${GO_VERSION}.linux-${goarch}.tar.gz
RUN tar -C /usr/ -xzf /tmp/go${GO_VERSION}.linux-${goarch}.tar.gz
ENV GOROOT=/usr/go
ENV PATH=\$PATH:\$GOROOT/bin:\$GOPATH/bin
"

	# Rust agent
	# rust installer should set path apropiately, just in case
	local cmake_file="cmake-${CMAKE_VERSION}.tar.gz"
	local cmake_dir="cmake-${CMAKE_VERSION}"
	readonly install_cmake="
RUN pushd /root; \
    curl -sLO https://github.com/Kitware/CMake/releases/download/v${CMAKE_VERSION}/${cmake_file}; \
	tar -zxf ${cmake_file}; \
	cd ${cmake_dir}; \
	./bootstrap > /dev/null 2>\&1; \
	make > /dev/null 2>\&1; \
	make install > /dev/null 2>\&1; \
	popd
"
	# install musl for compiling rust-agent
	install_musl=
	if [ "${muslarch}" == "aarch64" ]; then
		local musl_tar="${muslarch}-linux-musl-native.tgz"
		local musl_dir="${muslarch}-linux-musl-native"
		install_musl="
RUN cd /tmp; \
	curl -sLO https://musl.cc/${musl_tar}; tar -zxf ${musl_tar}; \
	 mkdir -p /usr/local/musl/; \
	cp -r ${musl_dir}/* /usr/local/musl/
ENV PATH=\$PATH:/usr/local/musl/bin
RUN ln -sf /usr/local/musl/bin/g++ /usr/bin/g++
"
	else
		local musl_tar="musl-${MUSL_VERSION}.tar.gz"
		local musl_dir="musl-${MUSL_VERSION}"
		install_musl="
RUN pushd /root; \
    curl -sLO https://www.musl-libc.org/releases/${musl_tar}; tar -zxf ${musl_tar}; \
	cd ${musl_dir}; \
	sed -i \"s/^ARCH = .*/ARCH = ${muslarch}/g\" dist/config.mak; \
	./configure > /dev/null 2>\&1; \
	make > /dev/null 2>\&1; \
	make install > /dev/null 2>\&1; \
	echo \"/usr/local/musl/lib\" > /etc/ld-musl-${muslarch}.path; \
	popd
ENV PATH=\$PATH:/usr/local/musl/bin
"
	fi

	readonly install_rust="
RUN curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSLf --output /tmp/rust-init; \
    chmod a+x /tmp/rust-init; \
	export http_proxy=${http_proxy:-}; \
	export https_proxy=${http_proxy:-}; \
	/tmp/rust-init -y
RUN . /root/.cargo/env; \
    export http_proxy=${http_proxy:-}; \
	export https_proxy=${http_proxy:-}; \
	cargo install cargo-when; \
	rustup toolchain install ${RUST_VERSION}; \
	rustup default ${RUST_VERSION}; \
	rustup target install ${rustarch}-unknown-linux-musl
RUN ln -sf /usr/bin/g++ /bin/musl-g++
"
	# rust agent still need go to build
	# because grpc-sys need go to build
	pushd ${dir}
	dockerfile_template="Dockerfile.in"
	dockerfile_arch_template="Dockerfile-${architecture}.in"
	# if arch-specific docker file exists, swap the univesal one with it.
        if [ -f "${dockerfile_arch_template}" ]; then
                dockerfile_template="${dockerfile_arch_template}"
        else
                [ -f "${dockerfile_template}" ] || die "${dockerfile_template}: file not found"
        fi

	# powerpc have no musl target, don't setup rust enviroment
	# since we cannot static link agent. Besides, there is
	# also long double representation problem when building musl-libc
	if [ "${architecture}" == "ppc64le" ] || [ "${architecture}" == "s390x" ]; then
		sed \
			-e "s|@GO_VERSION@|${GO_VERSION}|g" \
			-e "s|@OS_VERSION@|${OS_VERSION:-}|g" \
			-e "s|@INSTALL_CMAKE@||g" \
			-e "s|@INSTALL_MUSL@||g" \
			-e "s|@INSTALL_GO@|${install_go//$'\n'/\\n}|g" \
			-e "s|@INSTALL_RUST@||g" \
			-e "s|@SET_PROXY@|${set_proxy:-}|g" \
			${dockerfile_template} > Dockerfile
	else
		sed \
			-e "s|@GO_VERSION@|${GO_VERSION}|g" \
			-e "s|@OS_VERSION@|${OS_VERSION:-}|g" \
			-e "s|@INSTALL_CMAKE@|${install_cmake//$'\n'/\\n}|g" \
			-e "s|@INSTALL_MUSL@|${install_musl//$'\n'/\\n}|g" \
			-e "s|@INSTALL_GO@|${install_go//$'\n'/\\n}|g" \
			-e "s|@INSTALL_RUST@|${install_rust//$'\n'/\\n}|g" \
			-e "s|@SET_PROXY@|${set_proxy:-}|g" \
			${dockerfile_template} > Dockerfile
	fi
	popd
}

detect_go_version()
{
	info "Detecting agent go version"
	typeset yq=$(command -v yq || command -v ${GOPATH}/bin/yq || echo "${GOPATH}/bin/yq")
	if [ ! -f "$yq" ]; then
		source "$yq_file"
	fi

	local runtimeRevision=""

	# Detect runtime revision by fetching the agent's VERSION file
	local runtime_version_url="https://raw.githubusercontent.com/kata-containers/agent/${AGENT_VERSION:-master}/VERSION"
	info "Detecting runtime version using ${runtime_version_url}"

	if runtimeRevision="$(curl -fsSL ${runtime_version_url})"; then
		[ -n "${runtimeRevision}" ] || die "failed to get agent version"
		typeset -r runtimeVersionsURL="https://raw.githubusercontent.com/kata-containers/runtime/${runtimeRevision}/versions.yaml"
		info "Getting golang version from ${runtimeVersionsURL}"
		# This may fail if we are a kata bump.
		if GO_VERSION="$(curl -fsSL "$runtimeVersionsURL" | $yq r - "languages.golang.version")"; then
			[ "$GO_VERSION" != "null" ]
			return 0
		fi
	fi

	info "Agent version has not match with a runtime version, assumming it is a PR"
	local kata_runtime_pkg_dir="${GOPATH}/src/${GO_RUNTIME_PKG}"
	if [ ! -d "${kata_runtime_pkg_dir}" ];then
		info "There is not runtime repository in filesystem (${kata_runtime_pkg_dir})"
		local runtime_versions_url="https://raw.githubusercontent.com/kata-containers/runtime/${KATA_BRANCH}/versions.yaml"
		info "Get versions file from ${runtime_versions_url}"
		GO_VERSION="$(curl -fsSL "${runtime_versions_url}" | $yq r - "languages.golang.version")"
		if [ "$?" == "0" ] && [ "$GO_VERSION" != "null" ]; then
			return 0
		fi

		return 1
	fi

	local kata_versions_file="${kata_runtime_pkg_dir}/versions.yaml"
	info "Get Go version from ${kata_versions_file}"
	GO_VERSION="$(cat "${kata_versions_file}"  | $yq r - "languages.golang.version")"

	[ "$?" == "0" ] && [ "$GO_VERSION" != "null" ]
}

detect_rust_version()
{
	info "Detecting agent rust version"
	typeset -r yq=$(command -v yq || command -v ${GOPATH}/bin/yq || echo "${GOPATH}/bin/yq")
	if [ ! -f "$yq" ]; then
		source "$yq_file"
	fi

	local runtimeRevision=""

	# Detect runtime revision by fetching the agent's VERSION file
	local runtime_version_url="https://raw.githubusercontent.com/kata-containers/agent/${AGENT_VERSION:-master}/VERSION"
	info "Detecting runtime version using ${runtime_version_url}"

	if runtimeRevision="$(curl -fsSL ${runtime_version_url})"; then
		[ -n "${runtimeRevision}" ] || die "failed to get agent version"
		typeset -r runtimeVersionsURL="https://raw.githubusercontent.com/kata-containers/runtime/${runtimeRevision}/versions.yaml"
		info "Getting rust version from ${runtimeVersionsURL}"
		# This may fail if we are a kata bump.
		if RUST_VERSION="$(curl -fsSL "$runtimeVersionsURL" | $yq r - "languages.rust.version")"; then
			[ "$RUST_VERSION" != "null" ]
			return 0
		fi
	fi

	info "Agent version has not match with a runtime version, assumming it is a PR"
	local kata_runtime_pkg_dir="${GOPATH}/src/${GO_RUNTIME_PKG}"
	if [ ! -d "${kata_runtime_pkg_dir}" ];then
		info "There is not runtime repository in filesystem (${kata_runtime_pkg_dir})"
		local runtime_versions_url="https://raw.githubusercontent.com/kata-containers/runtime/${KATA_BRANCH}/versions.yaml"
		info "Get versions file from ${runtime_versions_url}"
		RUST_VERSION="$(curl -fsSL "${runtime_versions_url}" | $yq r - "languages.rust.version")"
		if [ "$?" == "0" ] && [ "$RUST_VERSION" != "null" ]; then
			return 0
		fi

		return 1
	fi

	local kata_versions_file="${kata_runtime_pkg_dir}/versions.yaml"
	info "Get rust version from ${kata_versions_file}"
	RUST_VERSION="$(cat "${kata_versions_file}"  | $yq r - "languages.rust.version")"

	[ "$?" == "0" ] && [ "$RUST_VERSION" != "null" ]
}

detect_cmake_version()
{
	info "Detecting cmake version"

	typeset -r yq=$(command -v yq || command -v ${GOPATH}/bin/yq || echo "${GOPATH}/bin/yq")
	if [ ! -f "$yq" ]; then
		source "$yq_file"
	fi

	local runtimeRevision=""

	# Detect runtime revision by fetching the agent's VERSION file
	local runtime_version_url="https://raw.githubusercontent.com/kata-containers/agent/${AGENT_VERSION:-master}/VERSION"
	info "Detecting runtime version using ${runtime_version_url}"

	if runtimeRevision="$(curl -fsSL ${runtime_version_url})"; then
		[ -n "${runtimeRevision}" ] || die "failed to get agent version"
		typeset -r runtimeVersionsURL="https://raw.githubusercontent.com/kata-containers/runtime/${runtimeRevision}/versions.yaml"
		info "Getting cmake version from ${runtimeVersionsURL}"
		# This may fail if we are a kata bump.
		if CMAKE_VERSION="$(curl -fsSL "$runtimeVersionsURL" | $yq r - "externals.cmake.version")"; then
			[ "$CMAKE_VERSION" != "null" ]
			return 0
		fi
	fi

	info "Agent version has not match with a runtime version, assumming it is a PR"
	local kata_runtime_pkg_dir="${GOPATH}/src/${GO_RUNTIME_PKG}"
	if [ ! -d "${kata_runtime_pkg_dir}" ];then
		info "There is not runtime repository in filesystem (${kata_runtime_pkg_dir})"
		local runtime_versions_url="https://raw.githubusercontent.com/kata-containers/runtime/${KATA_BRANCH}/versions.yaml"
		info "Get versions file from ${runtime_versions_url}"
		CMAKE_VERSION="$(curl -fsSL "${runtime_versions_url}" | $yq r - "externals.cmake.version")"
		if [ "$?" == "0" ] && [ "$CMAKE_VERSION" != "null" ]; then
			return 0
		fi

		return 1
	fi

	local kata_versions_file="${kata_runtime_pkg_dir}/versions.yaml"
	info "Get cmake version from ${kata_versions_file}"
	CMAKE_VERSION="$(cat "${kata_versions_file}"  | $yq r - "externals.cmake.version")"

	[ "$?" == "0" ] && [ "$CMAKE_VERSION" != "null" ]
}

detect_musl_version()
{
	info "Detecting musl version"

	typeset -r yq=$(command -v yq || command -v ${GOPATH}/bin/yq || echo "${GOPATH}/bin/yq")
	if [ ! -f "$yq" ]; then
		source "$yq_file"
	fi

	local runtimeRevision=""

	# Detect runtime revision by fetching the agent's VERSION file
	local runtime_version_url="https://raw.githubusercontent.com/kata-containers/agent/${AGENT_VERSION:-master}/VERSION"
	info "Detecting runtime version using ${runtime_version_url}"

	if runtimeRevision="$(curl -fsSL ${runtime_version_url})"; then
		[ -n "${runtimeRevision}" ] || die "failed to get agent version"
		typeset -r runtimeVersionsURL="https://raw.githubusercontent.com/kata-containers/runtime/${runtimeRevision}/versions.yaml"
		info "Getting musl version from ${runtimeVersionsURL}"
		# This may fail if we are a kata bump.
		if MUSL_VERSION="$(curl -fsSL "$runtimeVersionsURL" | $yq r - "externals.musl.version")"; then
			[ "$MUSL_VERSION" != "null" ]
			return 0
		fi
	fi

	info "Agent version has not match with a runtime version, assumming it is a PR"
	local kata_runtime_pkg_dir="${GOPATH}/src/${GO_RUNTIME_PKG}"
	if [ ! -d "${kata_runtime_pkg_dir}" ];then
		info "There is not runtime repository in filesystem (${kata_runtime_pkg_dir})"
		local runtime_versions_url="https://raw.githubusercontent.com/kata-containers/runtime/${KATA_BRANCH}/versions.yaml"
		info "Get versions file from ${runtime_versions_url}"
		MUSL_VERSION="$(curl -fsSL "${runtime_versions_url}" | $yq r - "externals.musl.version")"
		if [ "$?" == "0" ] && [ "$MUSL_VERSION" != "null" ]; then
			return 0
		fi

		return 1
	fi

	local kata_versions_file="${kata_runtime_pkg_dir}/versions.yaml"
	info "Get musl version from ${kata_versions_file}"
	MUSL_VERSION="$(cat "${kata_versions_file}"  | $yq r - "externals.musl.version")"

	[ "$?" == "0" ] && [ "$MUSL_VERSION" != "null" ]
}
