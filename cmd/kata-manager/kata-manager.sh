#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

[ -n "$DEBUG" ] && set -x

typeset -r script_name=${0##*/}

typeset -r kata_git_base="github.com/kata-containers"
typeset -r doc_repo="${kata_git_base}/documentation"
typeset -r test_repo="${kata_git_base}/tests"
typeset -r tarball_suffix="/archive/master.tar.gz"

typeset -r config_file_name="configuration.toml"
typeset -r default_image_dir="/usr/share/kata-containers"
typeset -r default_image_file="${default_image_dir}/kata-containers.img"
typeset -r default_initrd_file="${default_image_dir}/kata-containers-initrd.img"
typeset -r default_config_file="/usr/share/defaults/kata-containers/${config_file_name}"
typeset -r local_config_file="/etc/kata-containers/${config_file_name}"
typeset -r kata_doc_to_script="${test_repo}/.ci/kata-doc-to-script.sh"
# Downloaders list uses the format: [name of downloader]="downloader options"
typeset -r -A downloaders_list=([curl]="-fsSL" [wget]="-O -")
typeset -r default_docker_pkg_name="docker-ce"

# kernel boot option to enable agent debug
typeset -r agent_debug="agent.log=debug"

verbose="no"
execute="yes"
force="no"

# The default downloader to use
downloader=

# full path to the runtime configuration file to operate on
config_file=

# lower-case name of distribution
distro=

# Local path to kata git repositories, using the same hierarchy as GOPATH
kata_repos_base=

usage()
{
	cat <<EOT
Usage: ${script_name} [options] [command]

Description: Install and configure Kata Containers.

Options:

  -c <file> : Specify full path to configuration file
              (default: '$local_config_file').
  -f        : Force mode (for package removal).
  -h        : Display this help.
  -n        : No execute mode (a.k.a. dry run). Show the commands that kata-manager would run,
              without doing any change to the system.
  -v        : Verbose output.

Commands:

  configure-image       : Configure the runtime to use the specified image.
  configure-initrd      : Configure the runtime to use the specified initial ramdisk.
  disable-debug         : Turn off all debug options.
  enable-debug          : Turn on all debug options for all system components.
  install-docker        : Only install and configure Docker.
  install-docker-system : Install and configure Docker (implies 'install-packages').
  install-packages      : Install the packaged version of Kata Containers only.
  remove-docker         : Uninstall Docker only.
  remove-docker-system  : Uninstall Docker and Kata packages.
  remove-packages       : Uninstall the packaged version of Kata Containers.
  reset-config          : Undo changes to the runtime configuration [1].

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

	check_file "$file"

	config_checks

	sudo sed -i "s!^#*.*image *=.*\$!image = \"$file\"!g" "$config_file"

	egrep -q "\<image\> *=" "$config_file" && return

	# Add missing entry
	add_hypervisor_config "image" "\"$file\""
}

enable_initrd()
{
	local file="$1"

	check_file "$file"

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

	[ -z "$file" ] && file="${default_image_file}"

	info "installing '$file' as image"

	enable_image "$file"
	disable_initrd
}

cmd_configure_initrd()
{
	local file="$1"

	[ -z "$file" ] && file="${default_initrd_file}"

	info "installing '$file' as initrd"

	enable_initrd "$file"
	disable_image
}

detect_downloader()
{
	[ -n "$downloader" ] && return

	for d in ${!downloaders_list[@]}; do
		info "Checking downloader $d ..."

		if command -v $d >/dev/null; then
			downloader="$d ${downloaders_list[$d]}"
			return
		fi
	done

	die "Could not find a suitable http downloader (tried: ${!downloaders_list[@]})"
}

get_git_repo()
{
	local -r repo_path=$1

	[ -z "$repo_path" ] && die "No repo path specified"

	local -r local_dest="${kata_repos_base}/src/${repo_path}"
	if [ -d "$local_dest" ]; then
		info "repo $1 already available locally"
		return
	fi

	mkdir -p "$local_dest"
	local -r repo_url="https://${repo_path}"

	if ! command -v git >/dev/null; then
		info "getting repo $1 using http downloader"
		detect_downloader
		$downloader "${repo_url}/${tarball_suffix}" | tar xz -C "$local_dest" --strip-components=1
		return
	fi

	info "getting repo $1 using git"
	git clone "$repo_url" "$local_dest" || (rm -fr "$local_dest" && exit 1)
}

exec_document()
{
	local -r file="$1"
	local -r msg="$2"

	get_git_repo "$test_repo"

	local -r doc2script="${kata_repos_base}/src/${kata_doc_to_script}"

	[ ! -e "${doc2script}" ] && die "cannot find script ${doc2script}"

	local -r install_script=$(mktemp)

	# create the script
	"${doc2script}" "${file}" "${install_script}" "${msg}"

	if [ "$execute" = "no" ]
	then
		info "Not running script ${install_script} to $msg (created from document ${file})"

		# Note that we cannot exit since some commands run this
		# function multiple times.
		return
	fi

	info "$msg"

	# run the installation
	bash "${install_script}"

	# clean up
	rm -f "${install_script}"
}

# Install the packaged version of Kata by executing the commands
# specified in the installation guide document.
cmd_install_packages()
{
	get_git_repo "$doc_repo"

	local file="${distro}-installation-guide.md"

	local doc="${kata_repos_base}/src/${doc_repo}/install/${file}"
	[ ! -e "$doc" ] && die "no install document for distro $distro"

	exec_document "${doc}" "install packages for distro ${distro}"
}

install_container_manager()
{
	local mgr="$1"

	get_git_repo "$doc_repo"

	local file="install/${mgr}/${distro}-${mgr}-install.md"

	local doc="${kata_repos_base}/src/${doc_repo}/${file}"
	[ ! -e "$doc" ] && die "no ${mgr} install document for distro ${distro}"

	exec_document "${doc}" "install ${mgr} for distro ${distro}"
}

cmd_install_docker()
{
	install_container_manager "docker"
}

cmd_install_docker_system()
{
	cmd_install_packages
	cmd_install_docker
}

cmd_remove_packages()
{
	local packages_regex="${1:-^(kata|qemu-lite)-}"
	local packages

	info "removing packages"

	case "$distro" in
		centos|fedora|opensuse*|rhel|sles)
			packages=$(rpm -qa|egrep "${packages_regex}" || true)
			;;

		debian|ubuntu)
			packages=$(dpkg-query -W -f='${Package}\n'|egrep "${packages_regex}" || true)
			;;

		*) die "invalid distro: '$distro'" ;;
	esac

	[ -z "$packages" ] && die "packages not installed"

	if [ "$force" = "yes" ]
	then
		# Remove any locks held on the packages
		#
		# Note that these commands should not fail since:
		#
		# - There may be no lock on the package(s).
		# - In the case of Fedora/CentOS, the required packages to
		#   manipulate locks may not be installed. There is no point
		#   installing them though since if they are not installed,
		#   there can't be any locks on the packages so it's simplest to
		#   just let the commands fail silently.
		for pkg in $packages
		do
			case "$distro" in
				centos|rhel) sudo yum versionlock delete "$pkg" &>/dev/null || true ;;
				debian|ubuntu) sudo apt-mark unhold "$pkg" &>/dev/null || true ;;
				fedora) sudo dnf versionlock delete "$pkg" &>/dev/null || true ;;
				opensuse*|sles) sudo zypper removelock "$pkg" &>/dev/null || true ;;
			esac
		done
	fi

	case "$distro" in
		centos|rhel) sudo yum -y remove $packages ;;
		debian|ubuntu) sudo apt-get -y remove $packages ;;
		fedora) sudo dnf -y remove $packages ;;
		opensuse*|sles) sudo zypper remove -y $packages ;;
	esac
}

cmd_remove_docker()
{
	local docker_pkg="${KATA_DOCKER_PKG:-${default_docker_pkg_name}}"

	cmd_remove_packages "$docker_pkg"
}

cmd_remove_docker_system()
{
	cmd_remove_docker
	cmd_remove_packages
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
	source /etc/os-release || source /usr/lib/os-release
	distro=$ID

	kata_repos_base=$(go env GOPATH 2>/dev/null || true)
	if [ -z "$kata_repos_base" ]; then
		kata_repos_base="$HOME/go"
	fi
}

parse_args()
{
	config_file="${local_config_file}"

	while getopts "c:fhnv" opt
	do
		case "$opt" in
			c) config_file="$OPTARG" ;;
			f) force="yes" ;;
			h) usage && exit 0 ;;
			n) execute="no"; verbose="yes" ;;
			v) verbose="yes" ;;
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
		install-docker) cmd_install_docker ;;
		install-docker-system) cmd_install_docker_system ;;
		install-packages) cmd_install_packages ;;
		remove-docker) cmd_remove_docker ;;
		remove-docker-system) cmd_remove_docker_system ;;
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
