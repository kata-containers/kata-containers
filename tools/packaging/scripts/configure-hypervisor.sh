#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

#---------------------------------------------------------------------
# Description: This script is the *ONLY* place where "qemu*" build options
# should be defined.
#
# Note to maintainers:
#
# XXX: Every option group *MUST* be documented explaining why it has
# been specified.
#---------------------------------------------------------------------

script_name=${0##*/}

arch="${3:-$(uname -m)}"


# Array of configure options.
#
# Each element is comprised of two parts in the form:
#
#   tags:option
#
# Where,
#
# - 'tags' is a comma-separated list of values which denote why
#   the option is being specified.
#
# - 'option' is the hypervisor configuration option.
typeset -a qemu_options

typeset -A recognised_tags

# Prefix were kata will be installed
prefix=${PREFIX:-/usr}

# The QEMU version on "major.minor" format.
qemu_version=""

recognised_tags=(
	[arch]="architecture-specific"
	[functionality]="required functionality"
	[minimal]="specified to avoid building unnecessary elements"
	[misc]="miscellaneous"
	[security]="specified for security reasons"
	[size]="minimise binary size"
	[speed]="maximise startup speed"
)

# Given $1 and $2 as version strings with 'x.y.z' format; if $1 >= $2 then
# return 0. Otherwise return 1.
# Use this function on conditionals to compare versions.
#
gt_eq() {
	format='^[0-9]+(\.[0-9]+)*$'
	if [[ ! ("$1" =~ $format && "$2" =~ $format) ]]; then
		echo "ERROR: Malformed version string"
	fi
	echo -e "$1\n$2" | sort -V -r -C
}

# Display message to stderr and exit indicating script failed.
die() {
	local msg="$*"
	echo >&2 "$script_name: ERROR: $msg"
	exit 1
}

# Display usage to stdout.
usage() {
	cat <<EOF
Overview:

	Display configure options required to build the specified
	hypervisor.

Usage:

	$script_name [options] <hypervisor-name>

Options:

	-d : Dump all options along with the tags explaining why each option
	     is specified.
	-h : Display this help.
	-m : Display options one per line (includes continuation characters).
	-s : Generate options to build static

Example:

	$ $script_name qemu

EOF
}

show_tags_header() {
	local keys
	local key
	local value

	cat <<EOF
# Recognised option tags:
#
EOF

	# sort the tags
	keys=${!recognised_tags[@]}
	keys=$(echo "$keys" | tr ' ' '\n' | sort -u)

	for key in $keys; do
		value="${recognised_tags[$key]}"
		printf "#    %s\t%s.\n" "$key" "$value"
	done

	printf "#\n\n"
}

check_tag() {
	local tag="$1"
	local entry="$2"

	[ -z "$tag" ] && die "no tag for entry '$entry'"
	[ -z "$entry" ] && die "no entry for tag '$tag'"

	value="${recognised_tags[$tag]}"

	# each tag MUST have a description
	[ -n "$value" ] && return

	die "invalid tag '$tag' found for entry '$entry'"
}

check_tags() {
	local tags="$1"
	local entry="$2"

	[ -z "$tags" ] && die "entry '$entry' doesn't have any tags"
	[ -z "$entry" ] && die "no entry for tags '$tags'"

	tags=$(echo "$tags" | tr ',' '\n')

	for tag in $tags; do
		check_tag "$tag" "$entry"
	done
}

# Display an array to stdout.
#
# If 2 arguments are specified, split array across multiple lines,
# one per element with a backslash at the end of all lines except
# the last.
#
# Arguments:
#
# $1: *Name* of array variable (no leading '$'!!)
# $2: (optional) "multi" - show values across multiple lines,
#    "dump" - show full hash values. Any other value results in the
#    options being displayed on a single line.
show_array() {
	local action="$1"
	local _array=("$@")
	_array=("${_array[@]:1}")

	local -i size="${#_array[*]}"
	local -i i=1
	local entry
	local tags
	local elem
	local suffix
	local one_line="no"

	[ "$action" = "dump" ] && show_tags_header

	for entry in "${_array[@]}"; do
		[ -z "$entry" ] && die "found empty entry"

		tags=$(echo "$entry" | cut -s -d: -f1)
		elem=$(echo "$entry" | cut -s -d: -f2-)

		[ -z "$elem" ] && die "no option for entry '$entry'"

		check_tags "$tags" "$entry"

		if [ "$action" = "dump" ]; then
			printf "%s\t\t%s\n" "$tags" "$elem"
		elif [ "$action" = "multi" ]; then
			if [ $i -eq $size ]; then
				suffix=""
			else
				suffix=' \'
			fi

			printf '%s%s\n' "$elem" "$suffix"
		else
			one_line="yes"
			echo -n "$elem "
		fi

		i+=1
	done

	[ "$one_line" = yes ] && echo
}

generate_qemu_options() {
	#---------------------------------------------------------------------
	#check if cross-compile is needed
	host=$(uname -m)
	if [ $arch != $host ];then
		case $arch in
			aarch64) qemu_options+=(size:--cross-prefix=aarch64-linux-gnu-);;
			ppc64le) qemu_options+=(size:--cross-prefix=powerpc64le-linux-gnu-);;
			s390x) exit;;
			x86_64);;
			*) exit;;
		esac
	fi

	# Disabled options

	# braille support not required
	qemu_options+=(size:--disable-brlapi)

	# Don't build documentation
	qemu_options+=(minimal:--disable-docs)

	# Disable GUI (graphics)
	qemu_options+=(size:--disable-curses)
	qemu_options+=(size:--disable-gtk)
	qemu_options+=(size:--disable-opengl)
	qemu_options+=(size:--disable-sdl)
	qemu_options+=(size:--disable-spice)
	qemu_options+=(size:--disable-vte)

	# Disable graphical network access
	qemu_options+=(size:--disable-vnc)
	qemu_options+=(size:--disable-vnc-jpeg)
	if ! gt_eq "${qemu_version}" "7.0.50" ; then
		qemu_options+=(size:--disable-vnc-png)
	else
		qemu_options+=(size:--disable-png)
	fi

	qemu_options+=(size:--disable-vnc-sasl)

	# Disable PAM authentication: it's a feature used together with VNC access
	# that's not used. See QEMU commit 8953caf for more details
	qemu_options+=(size:--disable-auth-pam)

	# Disable unused filesystem support
	qemu_options+=(size:--disable-glusterfs)
	qemu_options+=(size:--disable-libiscsi)
	qemu_options+=(size:--disable-libnfs)

	# Starting from QEMU 4.1, libssh replaces to libssh2
	qemu_options+=(size:--disable-libssh)

	# Disable unused compression support
	qemu_options+=(size:--disable-bzip2)
	qemu_options+=(size:--disable-lzo)
	qemu_options+=(size:--disable-snappy)

	# Disable userspace network access ("-net user")
	qemu_options+=(size:--disable-slirp)

	# Disable USB
	qemu_options+=(size:--disable-libusb)
	qemu_options+=(size:--disable-usb-redir)

	# Disable TCG support
	case "$arch" in
	aarch64) ;;
	x86_64) qemu_options+=(size:--disable-tcg) ;;
	ppc64le) qemu_options+=(size:--disable-tcg) ;;
	s390x) qemu_options+=(size:--disable-tcg) ;;
	esac

	if [ "${static}" == "true" ]; then
		qemu_options+=(misc:--static)
	fi

	# Disable debug is always passed to the qemu binary so not required.
	case "$arch" in
	aarch64)
		;;
	x86_64)
		qemu_options+=(size:--disable-debug-tcg)
		qemu_options+=(size:--disable-tcg-interpreter)
		;;
	ppc64le)
		qemu_options+=(size:--disable-debug-tcg)
		qemu_options+=(size:--disable-tcg-interpreter)
		;;
	s390x)
		qemu_options+=(size:--disable-debug-tcg)
		qemu_options+=(size:--disable-tcg-interpreter)
		;;
	esac
	qemu_options+=(size:--disable-qom-cast-debug)

	# Disable libudev since it is only needed for qemu-pr-helper and USB,
	# none of which are used with Kata
	qemu_options+=(size:--disable-libudev)

	# Disallow network downloads
	qemu_options+=(security:--disable-curl)

	# Disable Remote Direct Memory Access (Live Migration)
	# https://wiki.qemu.org/index.php/Features/RDMALiveMigration
	qemu_options+=(size:--disable-rdma)

	# Don't build the qemu-io, qemu-nbd and qemu-image tools
	qemu_options+=(size:--disable-tools)

	# Kata Containers may be configured to use the virtiofs daemon.
	#
	# But since QEMU 5.2 the daemon is built as part of the tools set
	# (disabled with --disable-tools) thus it needs to be explicitely
	# enabled.
	#
	# From Kata Containers 2.5.0-alpha2 all arches but powerpc have been
	# using the new implementation of virtiofs daemon, which is not part
	# of QEMU.
	#
	# qemu configure does not support virtiofsd if qemu version >= 8.0.0.
        if ! gt_eq "${qemu_version}" "8.0.0" ; then
		qemu_options+=(functionality:--disable-virtiofsd)
        fi

	qemu_options+=(functionality:--enable-virtfs)

	# Don't build linux-user bsd-user
	qemu_options+=(size:--disable-bsd-user)
	qemu_options+=(size:--disable-linux-user)

	# Don't build sparse check tool
	qemu_options+=(size:--disable-sparse)

	# Don't build VDE networking backend
	qemu_options+=(size:--disable-vde)

	# Don't build other options which can't be depent on build server.
	if ! gt_eq "${qemu_version}" "7.0.50" ; then
		qemu_options+=(size:--disable-xfsctl)
		qemu_options+=(size:--disable-libxml2)
	fi
	qemu_options+=(size:--disable-nettle)

	# Disable XEN driver
	qemu_options+=(size:--disable-xen)

	# Disable Capstone
	qemu_options+=(size:--disable-capstone)

	# Disable graphics
	qemu_options+=(size:--disable-virglrenderer)

	# Disable block replication
	qemu_options+=(size:--disable-replication)

	# Disable USB smart card reader
	qemu_options+=(size:--disable-smartcard)

	# Disable guest agent
	qemu_options+=(size:--disable-guest-agent)
	qemu_options+=(size:--disable-guest-agent-msi)

	# unused image formats
	qemu_options+=(size:--disable-vvfat)
	qemu_options+=(size:--disable-vdi)
	qemu_options+=(size:--disable-qed)
	qemu_options+=(size:--disable-qcow1)
	qemu_options+=(size:--disable-bochs)
	qemu_options+=(size:--disable-cloop)
	qemu_options+=(size:--disable-dmg)
	qemu_options+=(size:--disable-parallels)

	# Disable new available features from 8.2.4
	qemu_options+=(size:--disable-colo-proxy)
	qemu_options+=(size:--disable-debug-graph-lock)
	qemu_options+=(size:--disable-hexagon-idef-parser)
	qemu_options+=(size:--disable-libdw)
	qemu_options+=(size:--disable-pipewire)
	qemu_options+=(size:--disable-pixman)
	qemu_options+=(size:--disable-relocatable)
	qemu_options+=(size:--disable-rutabaga-gfx)
	qemu_options+=(size:--disable-vmdk)
	qemu_options+=(size:--disable-avx512bw)
	qemu_options+=(size:--disable-vpc)
	qemu_options+=(size:--disable-vhdx)
	qemu_options+=(size:--disable-hv-balloon)

	# Disable various features based on the qemu_version
	if gt_eq "${qemu_version}" "9.1.0" ; then
		# Disable Query Processing Library support
		qemu_options+=(size:--disable-qpl)
		# Disable UADK Library support
		qemu_options+=(size:--disable-uadk)
		# Disable syscall buffer debugging support
		qemu_options+=(size:--disable-debug-remap)

	fi

	# Disable gio support
	qemu_options+=(size:--disable-gio)
	# Disable libdaxctl part of ndctl support
	qemu_options+=(size:--disable-libdaxctl)
	qemu_options+=(size:--disable-oss)

	# Building static binaries for aarch64 requires disabling PIE
	# We get an GOT overflow and the OS libraries are only build with fpic
	# and not with fPIC which enables unlimited sized GOT tables.
	if [ "${static}" == "true" ] && [ "${arch}" == "aarch64" ]; then
		qemu_options+=(arch:"--disable-pie")
	fi

	#---------------------------------------------------------------------
	# Enabled options

	# Enable kernel Virtual Machine support.
	# This is the default, but be explicit to avoid any future surprises
	qemu_options+=(speed:--enable-kvm)

	# Required for fast network access
	qemu_options+=(speed:--enable-vhost-net)

	# Support Linux AIO (native)
	qemu_options+=(size:--enable-linux-aio)
	qemu_options+=(size:--enable-linux-io-uring)

	# Support Ceph RADOS Block Device (RBD)
	[ -z "${static}" ] && qemu_options+=(functionality:--enable-rbd)

	# In "passthrough" security mode
	# (-fsdev "...,security_model=passthrough,..."), qemu uses a helper
	# application called virtfs-proxy-helper(1) to make certain 9p
	# operations safer.
	qemu_options+=(functionality:--enable-virtfs)
	qemu_options+=(functionality:--enable-attr)
	# virtio-fs needs cap-ng and seccomp
	qemu_options+=(functionality:--enable-cap-ng)
	qemu_options+=(functionality:--enable-seccomp)

	# AVX2 is enabled by default by x86_64, make sure it's enabled only
	# for that architecture
	if [ "$arch" == x86_64 ]; then
		qemu_options+=(speed:--enable-avx2)
		qemu_options+=(speed:--enable-avx512bw)
	else
		qemu_options+=(speed:--disable-avx2)
	fi
	# We're disabling pmem support, it is heavilly broken with
	# Ubuntu's static build of QEMU
	qemu_options+=(functionality:--disable-libpmem)

	# Enable libc malloc_trim() for memory optimization.
	qemu_options+=(speed:--enable-malloc-trim)

	#---------------------------------------------------------------------
	# Other options

	# 64-bit only
	if [ "${arch}" = "ppc64le" ]; then
		qemu_options+=(arch:"--target-list=ppc64-softmmu")
	else
		qemu_options+=(arch:"--target-list=${arch}-softmmu")
	fi

	# SECURITY: Create binary as a Position Independant Executable,
	# and take advantage of ASLR, making ROP attacks much harder to perform.
	# (https://wiki.debian.org/Hardening)
	[ -z "${static}" ] && qemu_options+=(arch:"--enable-pie")

	_qemu_cflags=""

	# compile with high level of optimisation
	# On version 5.2.0 onward the Meson build system warns to not use -O3
	_qemu_cflags+=" -O2"

	# Improve code quality by assuming identical semantics for interposed
	# synmbols.
	# Only enable if gcc is 5.3 or newer
	if gt_eq "${gcc_version}" "5.3.0" ; then
		_qemu_cflags+=" -fno-semantic-interposition"
	fi

	# Performance optimisation
	_qemu_cflags+=" -falign-functions=32"

	# SECURITY: make the compiler check for common security issues
	# (such as argument and buffer overflows checks).
	_qemu_cflags+=" -D_FORTIFY_SOURCE=2"

	# Set compile options
	qemu_options+=(functionality,security,speed,size:"--extra-cflags=\"${_qemu_cflags}\"")

	unset _qemu_cflags

	_qemu_ldflags=""

	# SECURITY: Disallow executing code on the stack.
	_qemu_ldflags+=" -z noexecstack"

	# SECURITY: Make the linker set some program sections to read-only
	# before the program is run to stop certain attacks.
	_qemu_ldflags+=" -z relro"

	# SECURITY: Make the linker resolve all symbols immediately on program
	# load.
	_qemu_ldflags+=" -z now"

	qemu_options+=(security:"--extra-ldflags=\"${_qemu_ldflags}\"")

	unset _qemu_ldflags

	# Where to install qemu helper binaries
	qemu_options+=(misc:--prefix=${prefix})

	# Where to install qemu libraries
	qemu_options+=(arch:--libdir=${prefix}/lib/${hypervisor})

	# Where to install qemu helper binaries
	qemu_options+=(misc:--libexecdir=${prefix}/libexec/${hypervisor})

	# Where to install data files
	qemu_options+=(misc:--datadir=${prefix}/share/${hypervisor})

}

# Entry point
main() {
	action=""

	while getopts "dhms" opt; do
		case "$opt" in
		d)
			action="dump"
			;;

		h)
			usage
			exit 0
			;;

		m)
			action="multi"
			;;
		s)
			static="true"
			;;
		esac
	done

	shift $((OPTIND - 1))

	[ -z "$1" ] && die "need hypervisor name"
	hypervisor="$1"

	local qemu_version_file="VERSION"
	[ -f ${qemu_version_file} ] || die "QEMU version file '$qemu_version_file' not found"

	# Remove any pre-release identifier so that it returns the version on
	# major.minor.patch format (e.g 5.2.0-rc4 becomes 5.2.0)
	qemu_version="$(awk 'BEGIN {FS = "-"} {print $1}' ${qemu_version_file})"

	[ -n "${qemu_version}" ] ||
		die "cannot determine qemu version from file $qemu_version_file"

	if ! gt_eq "${qemu_version}" "6.1.0" ; then
		die "Kata requires QEMU >= 6.1.0"
	fi

	local gcc_version_major=$(gcc -dumpversion | cut -f1 -d.)
	[ -n "${gcc_version_major}" ] ||
		die "cannot determine gcc major version, please ensure it is installed"
	# -dumpversion only returns the major version since GCC 7.0
	if gt_eq "${gcc_version_major}" "7.0.0" ; then
		local gcc_version_minor=$(gcc -dumpfullversion | cut -f2 -d.)
	else
		local gcc_version_minor=$(gcc -dumpversion | cut -f2 -d.)
	fi
	[ -n "${gcc_version_minor}" ] ||
		die "cannot determine gcc minor version, please ensure it is installed"
	local gcc_version="${gcc_version_major}.${gcc_version_minor}"

	# Generate qemu options
	generate_qemu_options

	show_array "$action" "${qemu_options[@]}"

	exit 0
}

main $@
