#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

[ -n "$DEBUG" ] && set -x

typeset -r script_name=${0##*/}

typeset -r doc_repo="github.com/kata-containers/documentation"
typeset -r test_repo="github.com/kata-containers/tests"

typeset -r config_file_name="configuration.toml"
typeset -r default_image_file="/usr/share/kata-containers/kata-containers.img"
typeset -r default_config_file="/usr/share/defaults/kata-containers/${config_file_name}"
typeset -r local_config_file="/etc/kata-containers/${config_file_name}"

# kernel boot option to enable agent debug
typeset -r agent_debug="agent.log=debug"

verbose="no"

# full path to the runtime configuration file to operate on
config_file=

# lower-case name of distribution
distro=

usage()
{
	cat <<EOT
Usage: ${script_name} [options] [command]

Description: Install and configure Kata Containers.

Options:

  -c <file> : Specify full path to configuration file
              (default: '$local_config_file').
  -h        : Display this help.
  -v        : Verbose output.

Commands:

  configure-image  : Configure the runtime to use the specified image.
  configure-initrd : Configure the runtime to use the specified initial ramdisk.
  disable-debug    : Turn off all debug options.
  enable-debug     : Turn on all debug options for all system components.
  install-packages : Install the packaged version of Kata Containers.
  remove-packages  : Uninstall the packaged version of Kata Containers.
  reset-config     : Undo changes to the runtime configuration [1].

Notes:

[1] - This is only possible if the user has not specified '-c'.


EOT
}

die()
{
	local msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

info()
{
	[ "$verbose" != "yes" ] && return

	local msg="$*"
	echo "INFO: $msg"
}

check_file()
{
	local file="$1"

	[ -z "$file" ] && die "need file"

	# Need to check as root to avoid EPERM which will cause the test to
	# fail
	sudo test ! -e "$file" && die "file '$file' does not exist" || true
}

config_checks()
{
	[ -e "$local_config_file" ] && return

	# The user has specified a non-standard config file so the only way to
	# handle this is to edit that file itself.
	[ "$config_file" != "$local_config_file" ] && return

	local dir=$(dirname "$local_config_file")

	sudo mkdir -p "$dir"

	# Create a copy of the default config file that this script can
	# operate on, but only if that file doesn't already exist.
	#
	# Note: these permissions match the currently packaged permissions
	if [ ! -e "$local_config_file" ]
	then
		sudo install -o root -g root -m 0644 \
			"$default_config_file" \
			"$local_config_file"

		# use an image by default
		cmd_configure_image "$default_image_file"
	fi
}

disable_image()
{
	config_checks

	sudo sed -i 's/^\(image *=.*\)/# \1/g' "$config_file"
}

disable_initrd()
{
	config_checks

	sudo sed -i 's/^\(initrd *=.*\)/# \1/g' "$config_file"
}

add_hypervisor_config()
{
	local name="$1"
	local value="$2"

	local -r hypervisor="qemu"

	sudo sed -i "/\[hypervisor\.${hypervisor}\]/a ${name} = $value" "$config_file"
}

enable_image()
{
	local file="$1"

	config_checks

	sudo sed -i "s!^#*.*image *=.*\$!image = \"$file\"!g" "$config_file"

	egrep -q "\<image\> *=" "$config_file" && return

	# Add missing entry
	add_hypervisor_config "image" "\"$file\""
}

enable_initrd()
{
	local file="$1"

	config_checks

	sudo sed -i "s!^#*.*initrd *=.*\$!initrd = \"$file\"!g" "$config_file"

	egrep -q "\<initrd\> *=" "$config_file" && return

	# Add missing entry
	add_hypervisor_config "initrd" "\"$file\""
}

cmd_enable_full_debug()
{
	info "enabling debug"

	config_checks

	sudo sed -i -e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' "$config_file"
	sudo sed -i -e "s/^kernel_params = \"\(.*\)\"/kernel_params = \"\1 ${agent_debug}\"/g" "$config_file"
}

cmd_disable_all_debug()
{
	info "disabling debug"

	config_checks

	sudo sed -i -e 's/^\(enable_debug.*=.*$\)/# \1/g' "$config_file"
	sudo sed -i -e "s/^\(kernel_params = \".*\)${agent_debug}\(.*\"\)/\1 \2/g" "$config_file"
}

cmd_configure_image()
{
	local file="$1"

	check_file "$file"

	info "installing '$file' as image"

	enable_image "$file"
	disable_initrd
}

cmd_configure_initrd()
{
	local file="$1"

	check_file "$file"

	info "installing '$file' as initrd"

	enable_initrd "$file"
	disable_image
}

# Install the packaged version of Kata by executing the commands
# specified in the installation guide document.
cmd_install_packages()
{
	command -v go >/dev/null || die "need golang"

	GOPATH=$(go env GOPATH)
	[ -z "${GOPATH}" ] && die "need GOPATH"

	source /etc/os-release

	local doc_repo_url="https://${doc_repo}"

	local repo_dir="${GOPATH}/src/${doc_repo}"

	info "installing packages"

	[ ! -d "${repo_dir}" ] && (cd "$(dirname ${repo_dir})" && git clone "$doc_repo_url")

	local file="${ID}-installation-guide.md"

	local doc="${GOPATH}/src/${doc_repo}/install/${file}"
	[ ! -e "$doc" ] && die "no install document for distro $distro"

	local -r doc_script="kata-doc-to-script.sh"

	local tool="${GOPATH}/src/${test_repo}/.ci/${doc_script}"
	[ ! -e "${tool}" ] && die "cannot find script $doc_script"

	local install_script=$(mktemp)

	# create the script
	"${tool}" "${doc}" "${install_script}"

	# run the installation
	bash "${install_script}"

	# clean up
	rm -f "${install_script}"
}

cmd_remove_packages()
{
	local packages_regex="^(kata|qemu-lite)-"
	local packages

	info "removing packages"

	case "$distro" in
		centos|fedora)
			packages=$(rpm -qa|egrep "${packages_regex}" || true)
			;;

		ubuntu)
			packages=$(dpkg-query -W -f='${Package}\n'|egrep "${packages_regex}" || true)
			;;

		*)
			die "invalid distro: '$distro'"
			;;
	esac

	[ -z "$packages" ] && die "packages not installed"

	case "$distro" in
		centos) sudo yum -y remove $packages ;;
		fedora) sudo dnf -y remove $packages ;;
		ubuntu) sudo apt-get -y remove $packages ;;
	esac
}

cmd_reset_config()
{
	# Cannot "undo" as the user has specified a custom config file
	[ "$config_file" != "$local_config_file" ] && die "cannot reset config file '$config_file'"

	sudo rm -f "$local_config_file"

	local dir=$(dirname "$local_config_file")
	sudo rmdir "$dir" 2>/dev/null || true
}

setup()
{
	source /etc/os-release

	distro=$ID
}

parse_args()
{
	config_file="${local_config_file}"

	while getopts "c:hv" opt
	do
		case "$opt" in
			c)
				config_file="$OPTARG"
				;;
			h)
				usage
				exit 0
				;;

			v)
				verbose="yes"
				;;
		esac
	done

	shift $[$OPTIND-1] || true
	cmd="$1"

	shift || true

	[ -z "$cmd" ] && usage && die "need command"

	case "$cmd" in
		configure-image) cmd_configure_image "$1" ;;
		configure-initrd) cmd_configure_initrd "$1" ;;
		disable-debug) cmd_disable_all_debug ;;
		enable-debug) cmd_enable_full_debug ;;
		install-packages) cmd_install_packages ;;
		remove-packages) cmd_remove_packages ;;
		reset-config) cmd_reset_config ;;
		*) usage && die "invalid command: '$cmd'" ;;
	esac
}

main()
{
    setup
    parse_args "$@"
}

main "$@"
