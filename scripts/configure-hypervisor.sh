#!/bin/bash
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

typeset -A recognised_tags

recognised_tags=(
	[arch]="architecture-specific"
	[functionality]="required functionality"
	[minimal]="specified to avoid building unnecessary elements"
	[misc]="miscellaneous"
	[security]="specified for security reasons"
	[size]="minimise binary size"
	[speed]="maximise startup speed"
)

# Display message to stderr and exit indicating script failed.
die()
{
	local msg="$*"
	echo >&2 "$script_name: ERROR: $msg"
	exit 1
}

# Display usage to stdout.
usage()
{
cat <<EOT
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

Example:

	$ $script_name qemu-lite

EOT
}

show_tags_header()
{
	local keys
	local key
	local value

	cat <<EOT
# Recognised option tags:
#
EOT

	# sort the tags
	keys=${!recognised_tags[@]}
	keys=$(echo "$keys"|tr ' ' '\n'|sort -u)

	for key in $keys
	do
		value="${recognised_tags[$key]}"
		printf "#    %s\t%s.\n" "$key" "$value"
	done

	printf "#\n\n"
}

check_tag()
{
	local tag="$1"
	local entry="$2"

	[ -z "$tag" ] && die "no tag for entry '$entry'"
	[ -z "$entry" ] && die "no entry for tag '$tag'"

	value="${recognised_tags[$tag]}"

	# each tag MUST have a description
	[ -n "$value" ] && return

	die "invalid tag '$tag' found for entry '$entry'"
}

check_tags()
{
	local tags="$1"
	local entry="$2"

	[ -z "$tags" ] && die "entry '$entry' doesn't have any tags"
	[ -z "$entry" ] && die "no entry for tags '$tags'"

	tags=$(echo "$tags"|tr ',' '\n')

	for tag in $tags
	do
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
show_array()
{
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

	for entry in "${_array[@]}"
	do
		[ -z "$entry" ] && die "found empty entry"

		tags=$(echo "$entry"|cut -s -d: -f1)
		elem=$(echo "$entry"|cut -s -d: -f2-)

		[ -z "$elem" ] && die "no option for entry '$entry'"

		check_tags "$tags" "$entry"

		if [ "$action" = "dump" ]
		then
			printf "%s\t\t%s\n" "$tags" "$elem"
		elif [ "$action" = "multi" ]
		then
			if [ $i -eq $size ]
			then
				suffix=""
			else
				suffix=" \\"
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

# Entry point
main()
{
	local qemu_version_file="VERSION"
	[ -f ${qemu_version_file} ] || die "QEMU version file '$qemu_version_file' not found"

	local qemu_version_major=$(cut -d. -f1 "${qemu_version_file}")
	local qemu_version_minor=$(cut -d. -f2 "${qemu_version_file}")

	[ -n "${qemu_version_major}" ] \
		|| die "cannot determine qemu major version from file $qemu_version_file"
	[ -n "${qemu_version_minor}" ] \
		|| die "cannot determine qemu minor version from file $qemu_version_file"

	local gcc_version_major=$(gcc -dumpversion | cut -f1 -d.)
	local gcc_version_minor=$(gcc -dumpversion | cut -f2 -d.)

	[ -n "${gcc_version_major}" ] \
		|| die "cannot determine gcc major version, please ensure it is installed"
	[ -n "${gcc_version_minor}" ] \
		|| die "cannot determine gcc minor version, please ensure it is installed"

	arch=$(arch)

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

	action=""

	while getopts "dhm" opt
	do
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
		esac
	done

	shift $[$OPTIND-1]

	[ -z "$1" ] && die "need hypervisor name"
	hypervisor="$1"

	#---------------------------------------------------------------------
	# Disabled options

	# bluetooth support not required
	qemu_options+=(size:--disable-bluez)

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
	qemu_options+=(size:--disable-vnc-png)
	qemu_options+=(size:--disable-vnc-sasl)

	# Disable unused filesystem support
	qemu_options+=(size:--disable-fdt)
	qemu_options+=(size:--disable-glusterfs)
	qemu_options+=(size:--disable-libiscsi)
	qemu_options+=(size:--disable-libnfs)
	qemu_options+=(size:--disable-libssh2)

	# Disable unused compression support
	qemu_options+=(size:--disable-bzip2)
	qemu_options+=(size:--disable-lzo)
	qemu_options+=(size:--disable-snappy)

	# Disable unused security options
	qemu_options+=(security:--disable-seccomp)
	qemu_options+=(security:--disable-tpm)

	# Disable userspace network access ("-net user")
	qemu_options+=(size:--disable-slirp)

	# Disable USB
	qemu_options+=(size:--disable-libusb)
	qemu_options+=(size:--disable-usb-redir)

	# Disable TCG support
	qemu_options+=(size:--disable-tcg)

	# SECURITY: Don't build a static binary (lowers security)
	# needed if qemu version is less than 2.7
	if [ "${qemu_version_major}" -eq 2 ] && [ "${qemu_version_minor}" -lt 7 ]; then
		qemu_options+=(security:--disable-static)
	fi

	# Not required as "-uuid ..." is always passed to the qemu binary
	qemu_options+=(size:--disable-uuid)

	# Disable debug
	qemu_options+=(size:--disable-debug-tcg)
	qemu_options+=(size:--disable-qom-cast-debug)
	qemu_options+=(size:--disable-tcg-interpreter)
	qemu_options+=(size:--disable-tcmalloc)

	# Disallow network downloads
	qemu_options+=(security:--disable-curl)

	# Disable Remote Direct Memory Access (Live Migration)
	# https://wiki.qemu.org/index.php/Features/RDMALiveMigration
	qemu_options+=(size:--disable-rdma)

	# Don't build the qemu-io, qemu-nbd and qemu-image tools
	qemu_options+=(size:--disable-tools)

	# Disable XEN driver
	qemu_options+=(size:--disable-xen)

	# FIXME: why is this disabled?
	# (for reference, it's explicitly enabled in Ubuntu 17.10 and
	# implicitly enabled in Fedora 27).
	qemu_options+=(size:--disable-linux-aio)

	#---------------------------------------------------------------------
	# Enabled options

	# Enable kernel Virtual Machine support.
	# This is the default, but be explicit to avoid any future surprises
	qemu_options+=(speed:--enable-kvm)

	# Required for fast network access
	qemu_options+=(speed:--enable-vhost-net)

	# Always strip binaries
	# needed if qemu version is less than 2.7
	if [ "${qemu_version_major}" -eq 2 ] && [ "${qemu_version_minor}" -lt 7 ]; then
		qemu_options+=(size:--enable-strip)
	fi

	# Support Ceph RADOS Block Device (RBD)
	qemu_options+=(functionality:--enable-rbd)

	# In "passthrough" security mode
	# (-fsdev "...,security_model=passthrough,..."), qemu uses a helper
	# application called virtfs-proxy-helper(1) to make certain 9p
	# operations safer.
	qemu_options+=(functionality:--enable-virtfs)
	qemu_options+=(functionality:--enable-attr)
	qemu_options+=(functionality:--enable-cap-ng)

	#---------------------------------------------------------------------
	# Other options

	# 64-bit only
	[ "$arch" = x86_64 ] && qemu_options+=(arch:"--target-list=${arch}-softmmu")

	_qemu_cflags=""

	# compile with high level of optimisation
	_qemu_cflags+=" -O3"

	# Improve code quality by assuming identical semantics for interposed
	# synmbols.
	# Only enable if gcc is 5.3 or newer
	if [ "${gcc_version_major}" -ge 5 ] && [ "${gcc_version_minor}" -ge 3 ]; then
		_qemu_cflags+=" -fno-semantic-interposition"
	fi

	# Performance optimisation
	_qemu_cflags+=" -falign-functions=32"

	# SECURITY: make the compiler check for common security issues
	# (such as argument and buffer overflows checks).
	_qemu_cflags+=" -D_FORTIFY_SOURCE=2"

	# SECURITY: Create binary as a Position Independant Executable,
	# and take advantage of ASLR, making ROP attacks much harder to perform.
	# (https://wiki.debian.org/Hardening)
	_qemu_cflags+=" -fPIE"

	# Set compile options
	qemu_options+=(functionality,security,speed,size:"--extra-cflags=\"${_qemu_cflags}\"")

	unset _qemu_cflags

	_qemu_ldflags=""

	# SECURITY: Link binary as a Position Independant Executable,
	# and take advantage of ASLR, making ROP attacks much harder to perform.
	# (https://wiki.debian.org/Hardening)
	_qemu_ldflags+=" -pie"

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

	# Where to install qemu libraries
	[ "$arch" = x86_64 ] && qemu_options+=(arch:--libdir=/usr/lib64/${hypervisor})

	# Where to install qemu helper binaries
	qemu_options+=(misc:--libexecdir=/usr/libexec/${hypervisor})

	# Where to install data files
	qemu_options+=(misc:--datadir=/usr/share/${hypervisor})

	show_array  "$action" "${qemu_options[@]}"

	exit 0
}

main $@
