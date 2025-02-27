#!/usr/bin/env bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

[ -n "${DEBUG:-}" ] && set -o xtrace

readonly script_name=${0##*/}

readonly kata_project="Kata Containers"
readonly containerd_project="containerd"

readonly kata_slug="kata-containers/kata-containers"
readonly containerd_slug="containerd/containerd"

readonly kata_project_url="https://github.com/${kata_slug}"
readonly containerd_project_url="https://github.com/${containerd_slug}"

readonly kata_hypervisors_doc_url='https://github.com/kata-containers/kata-containers/blob/main/docs/hypervisors.md'

readonly kata_releases_url="https://api.github.com/repos/${kata_slug}/releases"
readonly containerd_releases_url="https://api.github.com/repos/${containerd_slug}/releases"
readonly containerd_io_releases_url="https://raw.githubusercontent.com/containerd/containerd.io/main/content/releases.md"

readonly docker_slug="moby/moby"
readonly docker_project="Docker (moby)"
readonly docker_releases_url="https://api.github.com/repos/${docker_slug}/releases"

readonly nerdctl_slug="containerd/nerdctl"
readonly nerdctl_project="nerdctl"
readonly nerdctl_releases_url="https://api.github.com/repos/${nerdctl_slug}/releases"
readonly nerdctl_supported_arches="x86_64 aarch64"

readonly cni_project="cni-plugins"
readonly cni_slug="containernetworking/plugins"
readonly cni_releases_url="https://api.github.com/repos/${cni_slug}/releases"

# Directory created when unpacking a binary release archive downloaded from
# $kata_releases_url.
readonly kata_install_dir="${kata_install_dir:-/opt/kata}"

# containerd shim v2 details
readonly kata_runtime_name="kata"
readonly kata_runtime_type="io.containerd.${kata_runtime_name}.v2"
readonly kata_shim_v2="containerd-shim-${kata_runtime_name}-v2"
readonly kata_configuration="configuration"

readonly kata_clh_runtime_name="kata-clh"
readonly kata_clh_runtime_type="io.containerd.${kata_clh_runtime_name}.v2"
readonly kata_clh_shim_v2="containerd-shim-${kata_clh_runtime_name}-v2"
readonly kata_clh_configuration="configuration-clh"

# Systemd unit name for containerd daemon
readonly containerd_service_name="containerd.service"

# Containerd configuration file
readonly containerd_config="/etc/containerd/config.toml"

# Directory containing packaged configuration files
readonly kata_config_dir_golang="${kata_install_dir}/share/defaults/kata-containers"

# Directory containing local configuration files
readonly kata_local_config_dir_golang="/etc/kata-containers"

# Name of the Kata configuration file (or more usually the sym-link
# to the real config file).
readonly kata_config_file_name='configuration.toml'

readonly kata_config_file_golang="${kata_local_config_dir_golang}/${kata_config_file_name}"

# Currently, the golang runtime is the default runtime.
readonly default_config_dir="$kata_config_dir_golang"
readonly local_config_dir="$kata_local_config_dir_golang"
readonly kata_config_file="$kata_config_file_golang"
readonly kata_runtime_language='golang'
readonly pristine_config_type='packaged'

# The string to display to denote a default value.
readonly default_value='default'

# String to display if a value cannot be determined
readonly unknown_tag='unknown'

readonly cfg_file_install_perms='0644'

# Directory in which to create symbolic links
readonly link_dir=${link_dir:-/usr/bin}

readonly tmpdir=$(mktemp -d)

readonly warnings=$(cat <<EOF
WARNINGS:

- Use distro-packages where possible

  If your distribution packages $kata_project, you should use these packages rather
  than running this script.

- Packages will **not** be automatically updated

  Since a package manager is not being used, it is **your** responsibility
  to ensure these packages are kept up-to-date when new versions are released
  to ensure you are using a version that includes the latest security and bug fixes.

- Potentially untested versions or version combinations

  This script installs the *newest* versions of $kata_project
  and $containerd_project from binary release packages. These versions may
  not have been tested with your distribution version.

EOF
)

die()
{
	echo -e >&2 "ERROR: $*"
	exit 1
}

info()
{
	echo -e "INFO: $*"
}

cleanup()
{
	[ -d "$tmpdir" ] && rm -rf "$tmpdir"
}

# Determine the latest GitHub release for a project.
#
# Parameter: GitHub API URL for a projects releases.
# Note: Releases are assumed to use semver (https://semver.org) version
#   numbers.
github_get_latest_release()
{
	local url="${1:-}"

	# Notes:
	#
	# - The sort(1) call; none of the standard utilities support semver
	#   so attempt to perform a semver sort manually.
	# - Pre-releases are excluded via the select() call.
	local latest
	latest=$(curl -sL "$url" |\
		jq -r '.[].tag_name | select(contains("-") | not)' |\
		sort -t '.' -V |\
		tail -1 || true)

	[ -z "$latest" ] && die "Cannot determine latest release from $url"

	echo "$latest"
}

# Returns the actual version to download based on the specified one; if the
# specified version is blank, return the latest version.
github_resolve_version_to_download()
{
	local url="${1:-}"
	local requested_version="${2:-}"

	local version=""

	if [ -n "$requested_version" ]
	then
		version="$requested_version"
	else
		version=$(github_get_latest_release "$url" || true)
	fi

	echo "$version"
}

# Return the GitHub URL for a particular release's pre-built binary package.
#
# Parameters:
#
# - GitHub API URL for a project's releases.
# - Release version to look for.
github_get_release_file_url()
{
	local url="${1:-}"
	local version="${2:-}"

	# The version, less any leading 'v'
	local version_number
	version_number=${version#v}

	# Create an array to store architecture names
	local arches=()
	local arch=$(uname -m)

	arches+=("$arch")

	case "${arch}" in
		x86_64*)
                        arches+=("amd64") ;;
		aarch64*)
                        arches+=("arm64") ;;
		s390x*)
                        arches+=("s390x") ;;
		ppc64le*)
                        arches+=("ppc64le") ;;
		*)
			die "Unsupported arch. Must be x86_64, arm64, s390x, or ppc64le." ;;
	esac

	# Create a regular expression for matching
	local regex=""
	local arch_regex=$(IFS='|'; echo "${arches[*]}")
	arch_regex=$(printf "(%s)" "$arch_regex")

	case "$url" in
		*kata*)
			regex="kata-static-${version}-${arch_regex}.tar.xz" ;;
		*nerdctl*)
			# Keep this *always* before the containerd check, as it comes from
			# the very same containerd organisation on GitHub.
			regex="nerdctl-full-${version_number}-linux-${arch_regex}.tar.gz" ;;
		*containerd*)
			regex="containerd-${version_number}-linux-${arch_regex}.tar.gz" ;;
		*containernetworking*)
			regex="cni-plugins-linux-${arch_regex}-${version}.tgz" ;;

		*) die "invalid url: '$url'" ;;
	esac

	local download_url

	download_url=$(curl -sL "$url" |\
		jq --arg version "$version" \
		-r '.[] |
			select( (.tag_name == $version) or (.tag_name == "v" + $version) ) |
			.assets[].browser_download_url' |\
			grep -E "/${regex}$")

	download_url=$(echo "$download_url" | awk '{print $1}')

	[ -z "$download_url" ] && die "Cannot determine download URL for version $version ($url)"

	# Check to ensure there is only a single matching URL
	local expected_count=1

	local count
	count=$(echo "$download_url" | wc -l)

	[ "$count" -eq "$expected_count" ] || \
		die "expected $expected_count download URL but found $download_url"

	echo "$download_url"
}

# Download the release file for the specified projects release version and
# return the full path to the downloaded file.
#
# Parameters:
#
# - GitHub API URL for a project's releases.
# - Release version to download.
github_download_release()
{
	local url="${1:-}"
	local version="${2:-}"

	pushd "$tmpdir" >/dev/null

	local download_url
	download_url=$(github_get_release_file_url \
		"$url" \
		"$version" || true)

	[ -z "$download_url" ] && \
		die "Cannot determine download URL for version $version"

	# Don't specify quiet mode here so the user can observe download
	# progress.
	curl -LO "$download_url"

	local filename
	filename=$(echo "$download_url" | awk -F'/' '{print $NF}')

	ls -d "${PWD}/${filename}"

	popd >/dev/null
}

usage()
{
	cat <<EOF
Usage: $script_name [options] [<kata-version> [<containerd-version>]]

Description: Install $kata_project [1] (and optionally $containerd_project [2])
  from GitHub release binaries.

Options:

 -c <flavour>    : Specify containerd flavour ("lts" | "active" - default: "lts").
                   Find more details on LTS and Active versions of containerd on
                   https://containerd.io/releases/#support-horizon
 -d              : Enable debug for all components.
 -D              : Install Docker server and CLI tooling (takes priority over '-c').
 -e              : List short names and details for local hypervisor configuration files (for '-S').
 -f              : Force installation (use with care).
 -h              : Show this help statement.
 -H <hypervisor> : Specify the hypervisor name to use when *INSTALLING* a system.
 -k <version>    : Specify Kata Containers version.
 -K <tarball>    : Specify local Kata Containers tarball to install (takes priority over '-k').
 -l              : List installed and available versions only, then exit (uses network).
 -L              : List short names and details for official packaged hypervisor configurations (for '-S').
 -N              : Install nerdctl (takes priority over '-c', only implemented for x86_64 and ARM).
 -o              : Only install Kata Containers.
 -r              : Don't cleanup on failure (retain files).
 -S <hypervisor> : Only change the hypervisor config for an *EXISTING* installation.
 -t              : Disable self test (don't try to create a container after install).
 -T              : Only run self test (do not install anything).

Notes:

- The version strings must refer to official GitHub releases for each project.
  If not specified or set to "", install the latest available version.

- The '-L' option requires an installed system.

  > **Note:** For details of each hypervisor, see [3].

- If an invalid hypervisor name is specified with the '-H' option, an
  error will be generated, but the system will be left in a working
  state and configured to use the hypervisor configured at build time.

  > **Note:** For details of each hypervisor, see [3].

- Since '-L' cannot be used until a system is installed, if you wish
  to change the configured Kata hypervisor, unless you know the
  hypervisor name, the recommended approach is to:

  1) Install this script with no arguments to install a Kata system.
  2) Run again with '-L' to list the available hypervisor configurations.
  3) Run again with '-S <hypervisor>' to switch to your chosen hypervisor.

See also:

[1] - $kata_project_url
[2] - $containerd_project_url
[3] - $kata_hypervisors_doc_url

$warnings

Advice:

- You can check the latest version of Kata Containers by running
  one of the following:

  $ $script_name -l
  $ kata-runtime check --only-list-releases
  $ kata-ctl check only-list-releases

EOF
}

# Return 0 if containerd is already installed, else return 1.
containerd_installed()
{
	command -v containerd &>/dev/null && return 0

	systemctl list-unit-files --type service |\
		grep -Eq "^${containerd_service_name}\>" \
		&& return 0

	return 1
}

# Return 0 if Kata is already installed, else return 1.
kata_installed()
{
	command -v "$kata_shim_v2" &>/dev/null
}

# Assert that Kata is installed and error with a message if it isn't.
ensure_kata_already_installed()
{
	local msg
	msg=$(cat <<-EOF
		$kata_project is not yet installed (or is installed in a non-standard directory).

		Run this script with no options to install Kata Containers.
	EOF
	)

	kata_installed || die "$msg"
}

pre_checks()
{
	info "Running pre-checks"

	local skip_containerd="${1:-}"
	[ -z "$skip_containerd" ] && die "no skip_containerd value"

	command -v "${kata_shim_v2}" &>/dev/null \
		&& die "Please remove existing $kata_project installation"

	[ "$skip_containerd" = 'true' ] && return 0

	local ret

	{ containerd_installed; ret=$?; } || true

	[ "$ret" -eq 0 ] && die "$containerd_project already installed"

	return 0
}

check_deps()
{
	info "Checking dependencies"

	# Maps command names to package names using a colon delimiter
	local elems=()

	elems+=("curl:curl")
	elems+=("git:git")
	elems+=("jq:jq")
	elems+=("tar:tar")

	local pkgs_to_install=()

	local elem

	for elem in "${elems[@]}"
	do
		local cmd
		cmd=$(echo "$elem"|cut -d: -f1)

		local pkg
		pkg=$(echo "$elem"|cut -d: -f2-)

		command -v "$cmd" &>/dev/null && continue

		pkgs_to_install+=("$pkg")
	done

	[ "${#pkgs_to_install[@]}" -eq 0 ] && return 0

	local packages
	packages="${pkgs_to_install[@]}"

	info "Installing packages '$packages'"

	case "$ID" in
		centos|rhel) sudo yum -y install $packages ;;
		debian|ubuntu) sudo apt-get update && sudo apt-get -y install $packages ;;
		fedora) sudo dnf -y install $packages ;;
		opensuse*|sles) sudo zypper install -y $packages ;;
		*) die "Cannot automatically install packages on $ID, install $packages manually and re-run"
	esac
}

setup()
{
	local cleanup="${1:-}"
	[ -z "$cleanup" ] && die "no cleanup value"

	local force="${2:-}"
	[ -z "$force" ] && die "no force value"

	local skip_containerd="${3:-}"
	[ -z "$skip_containerd" ] && die "no skip_containerd value"

	[ "$cleanup" = "true" ] && trap cleanup EXIT

	source /etc/os-release || source /usr/lib/os-release

	#these dependencies are needed inside this script, and should be checked regardless of the -f option.
	check_deps

	[ "$force" = "true" ] && return 0

	pre_checks "$skip_containerd"
}

# Download the requested version of the specified project.
#
# Returns the resolve version number and the full path to the downloaded file
# separated by a colon.
github_download_package()
{
	local releases_url="${1:-}"
	local requested_version="${2:-}"

	# Only used for error message
	local project="${3:-}"

	[ -z "$releases_url" ] && die "need releases URL"
	[ -z "$project" ] && die "need project URL"

	local version
	version=$(github_resolve_version_to_download \
		"$releases_url" \
		"$requested_version" || true)

	[ -z "$version" ] && die "Unable to determine $project version to download"

	local file
	file=$(github_download_release \
		"$releases_url" \
		"$version")
	echo "${version}:${file}"
}

install_containerd()
{
	local requested_version="${1:-}"

	local project="$containerd_project"

	local version_desc="latest version"
	[ -n "$requested_version" ] && version_desc="version $requested_version"

	info "Downloading $project release ($version_desc)"

	local results
	results=$(github_download_package \
		"$containerd_releases_url" \
		"$requested_version" \
		"$project")

	[ -z "$results" ] && die "Cannot download $project release file"

	local version
	version=$(echo "$results"|cut -d: -f1)

	local file
	file=$(echo "$results"|cut -d: -f2-)

	[ -z "$version" ] && die "Cannot determine $project resolved version"
	[ -z "$file" ] && die "Cannot determine $project release file"

	info "Installing $project release $version from $file"

	sudo tar -C /usr/local -xvf "${file}"

	for file in \
		/usr/local/bin/containerd \
		/usr/local/bin/ctr
		do
			sudo ln -sf "$file" "${link_dir}"
		done

	info "$project installed\n"

	# Installing cni plugins for containerd
	install_cni
}

install_nerdctl()
{
	local project="$nerdctl_project"

	info "Downloading $project latest release"

	local results
	results=$(github_download_package \
		"$nerdctl_releases_url" \
		"" \
		"$project")

	[ -z "$results" ] && die "Cannot download $project release file"

	local version
	version=$(echo "$results"|cut -d: -f1)

	local file
	file=$(echo "$results"|cut -d: -f2-)

	[ -z "$version" ] && die "Cannot determine $project resolved version"
	[ -z "$file" ] && die "Cannot determine $project release file"

	info "Installing $project release $version from $file"

	sudo tar -C /usr/local -xvf "${file}"
	for file in \
		/usr/local/bin/containerd \
		/usr/local/bin/ctr \
		/usr/local/bin/nerdctl \
		/usr/local/bin/runc \
		/usr/local/bin/slirp4netns
		do
			sudo ln -sf "$file" "${link_dir}"
		done

	info "$project installed\n"

	sudo mkdir -p /opt/cni/bin

	# nerdctl requires cni plugins to be installed in /opt/cni/bin
	# Copy extracted tarball cni files under /usr/local/libexec
	sudo cp /usr/local/libexec/cni/* /opt/cni/bin/

	info "cni plugins installed under /opt/cni/bin"
}

install_cni()
{
	local project="$cni_project"

	info "Downloading $project latest release"

	local results
	results=$(github_download_package \
		"$cni_releases_url" \
		"" \
		"$project")

	[ -z "$results" ] && die "Cannot download $project release file"

	local version
	version=$(echo "$results"|cut -d: -f1)

	local file
	file=$(echo "$results"|cut -d: -f2-)

	[ -z "$version" ] && die "Cannot determine $project resolved version"
	[ -z "$file" ] && die "Cannot determine $project release file"

	info "Installing $project release $version from $file"

	sudo mkdir -p /opt/cni/bin

	sudo tar -C /opt/cni/bin -xvf "${file}"

	info "$project installed\n"
}

configure_containerd()
{
	local enable_debug="${1:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"
	local configure_systemd_service="${2:-true}"

	local project="$containerd_project"

	info "Configuring $project"

	if [ "$configure_systemd_service" = "true" ]
	then
		local systemd_unit_dir="/etc/systemd/system"
		sudo mkdir -p "$systemd_unit_dir"

		local dest="${systemd_unit_dir}/${containerd_service_name}"

		if [ ! -f "$dest" ]
		then
			pushd "$tmpdir" >/dev/null

			local service_url
			service_url=$(printf "%s/%s/%s/%s" \
				"https://raw.githubusercontent.com" \
				"${containerd_slug}" \
				"main" \
				"${containerd_service_name}")

			curl -LO "$service_url"

			printf "# %s: Service installed for Kata Containers\n" \
				"$(date -Iseconds)" |\
				tee -a "$containerd_service_name"

			sudo cp "${containerd_service_name}" "${dest}"
			sudo systemctl daemon-reload

			info "Installed ${dest}"

			popd >/dev/null
		fi
	fi

	# Backup the original containerd configuration:
	sudo mkdir -p "$(dirname $containerd_config)"

	sudo test -e "$containerd_config" || {
		sudo touch "$containerd_config"
		info "Created $containerd_config"
	}

	local original
	original="${containerd_config}-pre-kata-$(date -I)"

	sudo grep -q "$kata_runtime_type" "$containerd_config" || {
		sudo cp "$containerd_config" "${original}"
		info "Backed up containerd config file '$containerd_config' to '$original'"
	}

	local modified="false"

	# Add the Kata Containers configuration details:

	local comment_text
	comment_text=$(printf "%s: Added by %s\n" \
		"$(date -Iseconds)" \
		"$script_name")

	sudo grep -q "$kata_runtime_type" "$containerd_config" || {
		cat <<-EOF | sudo tee -a "$containerd_config"
		# $comment_text
		[plugins]
		  [plugins."io.containerd.grpc.v1.cri"]
		    [plugins."io.containerd.grpc.v1.cri".containerd]
		      default_runtime_name = "${kata_runtime_name}"
		      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes]
		        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.${kata_runtime_name}]
		          runtime_type = "${kata_runtime_type}"
			  privileged_without_host_devices = true
			  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.${kata_runtime_name}.options]
			    ConfigPath = "/opt/kata/share/defaults/kata-containers/${kata_configuration}.toml"
		        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.${kata_clh_runtime_name}]
		          runtime_type = "${kata_clh_runtime_type}"
			  privileged_without_host_devices = true
			  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.${kata_clh_runtime_name}.options]
			    ConfigPath = "/opt/kata/share/defaults/kata-containers/${kata_clh_configuration}.toml"
		EOF

		modified="true"
	}

	if [ "$enable_debug" = "true" ]
	then
		local debug_enabled
		debug_enabled=$(awk -v RS='' '/\[debug\]/' "$containerd_config" |\
			grep -E "^\s*\<level\>\s*=\s*.*\<debug\>" || true)

		[ -n "$debug_enabled" ] || {
			cat <<-EOF | sudo tee -a "$containerd_config"
			# $comment_text
			[debug]
				level = "debug"
			EOF
		}

		modified="true"
	fi

	[ "$modified" = "true" ] && info "Modified containerd config file '$containerd_config'"
	sudo systemctl daemon-reload
	sudo systemctl enable containerd
	sudo systemctl start containerd

	local msg="disabled"
	[ "$enable_debug" = "true" ] && msg="enabled"

	info "Configured $project (debug $msg)\n"
}

install_kata()
{
	local requested_version="${1:-}"
	local kata_tarball="${2:-}"

	local project="$kata_project"

	local version=""
	if [ -z "$kata_tarball" ]
	then
		local version_desc="latest version"
		[ -n "$requested_version" ] && version_desc="version $requested_version"

		info "Downloading $project release ($version_desc)"

		local results
		results=$(github_download_package \
			"$kata_releases_url" \
			"$requested_version" \
			"$project")

		[ -z "$results" ] && die "Cannot download $project release file"

		version=$(echo "$results"|cut -d: -f1)

		[ -z "$version" ] && die "Cannot determine $project resolved version"

		local file
		file=$(echo "$results"|cut -d: -f2-)
	else
		file="$kata_tarball"
	fi

	[ -z "$file" ] && die "Cannot determine $project release file"

	# Allow the containerd service to find the Kata shim and users to find
	# important commands:
	local create_links_for=()

	create_links_for+=("$kata_shim_v2")
	create_links_for+=("kata-collect-data.sh")
	create_links_for+=("kata-runtime")

	local from_dir
	from_dir=$(printf "%s/bin" "$kata_install_dir")

	if [ -n "$kata_tarball" ]
	then
		info "Installing $project release from $file"
	else
		# Only do the checking in case the tarball was not explicitly passed
		# by the user.  We have no control of what's passed and we cannot
		# expect that all the files are going to be under /opt.
		info "Checking file '$file'"

		# Since we're unpacking to the root directory, perform a sanity check
		# on the archive first.
		local unexpected
		unexpected=$(tar -tf "${file}" |\
			grep -Ev "^(\./$|\./opt/$|\.${kata_install_dir}/)" || true)

		[ -n "$unexpected" ] && die "File '$file' contains unexpected paths: '$unexpected'"

		info "Installing $project release $version from $file"
	fi

	sudo tar -C / -xvf "${file}"

	[ -d "$from_dir" ] || die "$project does not exist in expected directory $from_dir"

	for file in "${create_links_for[@]}"
	do
		local from_path
		from_path=$(printf "%s/%s" "$from_dir" "$file")
		[ -e "$from_path" ] || die "File $from_path not found"

		sudo ln -sf "$from_path" "$link_dir"
	done

	local tdx_qemu_config="/opt/kata/share/defaults/kata-containers/configuration-qemu-tdx.toml"
	local tdx_qemu_path_from_distro="NOT_SUPPORTED"
	local tdx_ovmf_path_from_distro="NOT_SUPPORTED"
	if [ -e $tdx_qemu_config ]; then
		source /etc/os-release || source /usr/lib/os-release
		case $ID in
			ubuntu)
				case $VERSION_ID in
					24.04)
						tdx_qemu_path_from_distro="/usr/bin/qemu-system-x86_64"
						tdx_ovmf_path_from_distro="/usr/share/ovmf/OVMF.fd"
						;;
				esac
				;;
			centos)
				case $VERSION_ID in
					9)
						tdx_qemu_path_from_distro="/usr/libexec/qemu-kvm"
						tdx_ovmf_path_from_distro="/usr/share/edk2/ovmf/OVMF.inteltdx.fd"
						;;
				esac
				;;
		esac

		sudo sed -i -e "s|PLACEHOLDER_FOR_DISTRO_QEMU_WITH_TDX_SUPPORT|$tdx_qemu_path_from_distro|g" $tdx_qemu_config
		sudo sed -i -e "s|PLACEHOLDER_FOR_DISTRO_OVMF_WITH_TDX_SUPPORT|$tdx_ovmf_path_from_distro|g" $tdx_qemu_config
	fi

	info "$project installed\n"
}

configure_kata()
{
	local enable_debug="${1:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	local force="${2:-}"
	[ -z "$force" ] && die "no force value"

	local hypervisor="${3:-}"

	[ "$enable_debug" = "false" ] && \
		info "Using default $kata_project configuration" && \
		return 0

	local default_hypervisor
	default_hypervisor=$(get_default_packaged_hypervisor || true)

	[ -z "$hypervisor" ] && \
		hypervisor="$default_hypervisor" && \
		info "Using default $kata_project hypervisor ('$hypervisor')"

	set_kata_config_file "$force" "$hypervisor"

	local cfg_file="$kata_config_file"

	# Note that '--follow-symlinks' is essential: without it,
	# sed(1) will break the sym-link and convert it into a file,
	# which is not desirable behaviour as the whole point of the
	# "well known" config file name is that it is a sym-link to
	# the actual config file.
	#
	# However, this option is GNU sed(1) specific so if this
	# script is run on a non-Linux system, it may be necessary
	# to install GNU sed, or find an equivalent option for the
	# local version of sed.
	sudo sed -i \
		--follow-symlinks \
		-e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' \
		-e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug"/g' \
		"$cfg_file"

	info "Configured $kata_project config file '$cfg_file' for full debug"
	info "(delete '$cfg_file' to use pristine $kata_project configuration)"
}

handle_kata()
{
	local version="${1:-}"
	local tarball="${2:-}"

	local enable_debug="${3:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	local force="${4:-}"
	[ -z "$force" ] && die "no force value"

	local hypervisor="${5:-}"

	install_kata "$version" "$tarball"

	configure_kata "$enable_debug" "$force" "$hypervisor"

	kata-runtime --version
}

containerd_version_number()
{
	local flavour="${1:-}"
	[ -z "$flavour" ] && die "need containerd flavour"

	base_version="$(curl -fsSL $containerd_io_releases_url | \
		grep -iE "\[d+.d+\].*| $flavour" | \
		grep -oE "\[[0-9]+.[0-9]+\]" | \
		grep -oE "[0-9]+.[0-9]+")"

	curl --silent ${containerd_releases_url} | \
		jq -r .[].tag_name | \
		grep "^v${base_version}.[0-9]*$" -m1
}

handle_containerd()
{
	local flavour="${1:-}"
	[ -z "$flavour" ] && die "need containerd flavour"

	local force="${2:-}"
	[ -z "$force" ] && die "need force value"

	local enable_debug="${3:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	local version="$(containerd_version_number "$flavour")"
	local ret

	if [ "$force" = "true" ]
	then
		install_containerd "$version"
	else
		{ containerd_installed; ret=$?; } || true

		if [ "$ret" -eq 0 ]
		then
			info "Using existing containerd installation"
		else
			install_containerd "$version"
		fi
	fi

	configure_containerd "$enable_debug"

	containerd --version
}

handle_docker()
{
	{ containerd_installed; ret=$?; } || true
	if [ "$ret" -eq 0 ]
	then
		info "Backing up previous $containerd_project configuration"
		[ -e "$containerd_config" ] && sudo mv $containerd_config $containerd_config.system-$(date -Iseconds)
	fi

	local filename='get-docker.sh'

	local file
	file="$tmpdir/$filename"

	curl -fsSL https://get.docker.com -o "$file"
	sudo sh "$file"

	rm -rf "$file"

	sudo systemctl enable --now docker

	configure_containerd "$enable_debug"

	containerd --version
	docker --version
}

handle_nerdctl()
{
	{ containerd_installed; ret=$?; } || true
	if [ "$ret" -eq 0 ]
	then
		info "Backing up previous $containerd_project configuration"
		[ -e "$containerd_config" ] && sudo mv $containerd_config $containerd_config.system-$(date -Iseconds)
	fi

	install_nerdctl

	configure_containerd "$enable_debug" "false"

	sudo systemctl enable --now containerd

	containerd --version
	nerdctl --version
}

test_installation()
{
	local tool="${1:-}"
	[ -z "$tool" ] && die "The tool to test $kata_project with was not specified"

	info "Testing $kata_project\n"

	sudo kata-runtime check -v

	local image="quay.io/prometheus/busybox:latest"
	sudo $tool image pull "$image"

	local container_name="${script_name/./-}-test-kata"

	# Used to prove that the kernel in the container
	# is different to the host kernel.
	cmd="sudo $tool run --runtime "$kata_runtime_type" --rm"
	case "$tool" in
		docker)
			# docker takes the container name as `--name
			# $container_name`, passed to the run option.
		       	cmd+=" --name $container_name" ;;
	esac
	cmd+=" $image"
	case "$tool" in
		ctr)
			# ctr takes the container name as a mandatory
			# argument after the image name
			cmd+=" $container_name" ;;
	esac
	cmd+=" uname -r"

	info "Running \"$cmd\""
	container_kernel=$(eval "$cmd" || true)

	[ -z "$container_kernel" ] && die "Failed to test $kata_project"

	local host_kernel
	host_kernel=$(uname -r)

	info "Test successful:\n"

	info "  Host kernel version      : $host_kernel"
	info "  Container kernel version : $container_kernel"
	echo
}

handle_installation()
{
	local cleanup="${1:-}"
	[ -z "$cleanup" ] && die "no cleanup value"

	local force="${2:-}"
	[ -z "$force" ] && die "no force value"

	local skip_containerd="${3:-}"
	[ -z "$skip_containerd" ] && die "no only Kata value"

	local enable_debug="${4:-}"
	[ -z "$enable_debug" ] && die "no enable debug value"

	local disable_test="${5:-}"
	[ -z "$disable_test" ] && die "no disable test value"

	local only_run_test="${6:-}"
	[ -z "$only_run_test" ] && die "no only run test value"

	# These params can be blank
	local kata_version="${7:-}"
	local containerd_flavour="${8:-}"
	local install_docker="${9:-}"
	local install_nerdctl="${10:-}"
	[ -z "$install_docker" ] && die "no install docker value"
	[ -z "$install_nerdctl" ] && die "no install nerdctl value"

	local kata_tarball="${11:-}"
	local hypervisor="${12:-}"

	# The tool to be testing the installation with
	local tool="ctr"

	if [ "$install_docker" = "true" ]
	then
		if [ "$skip_containerd" = "false" ]
		then
			# The script provided by docker already takes care
			# of properly installing containerd
			skip_containerd="true"
			info "Containerd will be installed during the Docker installation ('-c' option ignored)"
		fi
		tool="docker"
	fi

	if [ "$install_nerdctl" = "true" ]
	then
		local arch=$(uname -m)
		if ! grep -q " $arch " <<< " $nerdctl_supported_arches "
		then
			die "nerdctl deployment only supports $nerdctl_supported_arches"
		fi

		if [ "$skip_containerd" = "false" ]
		then
			# The script provided by nerdctl already takes care
			# of properly installing containerd
			skip_containerd="true"
			info "Containerd will be installed during the nerdctl installation ('-c' option ignored)"
		fi
		tool="nerdctl"
	fi

	[ "$only_run_test" = "true" ] && test_installation "$tool"  && return 0

	setup "$cleanup" "$force" "$skip_containerd"

	handle_kata \
		"$kata_version" \
		"$kata_tarball" \
		"$enable_debug" \
		"$force" \
		"$hypervisor"

	[ "$skip_containerd" = "false" ] && \
		handle_containerd \
		"$containerd_flavour" \
		"$force" \
		"$enable_debug"

	[ "$install_docker" = "true" ] && [ "$install_nerdctl" = "true" ] && \
		die "Installing docker and nerdctl at the same time is not possible."

	[ "$install_docker" = "true" ] && handle_docker

	[ "$install_nerdctl" = "true" ] && handle_nerdctl

	[ "$disable_test" = "false" ] && test_installation "$tool"

	if [ "$skip_containerd" = "true" ] && ( [ "$install_docker" = "false" ] || [ "$install_nerdctl" = "false" ] )
	then
		info "$kata_project is now installed"
	else
		local extra_projects="containerd"
		[ "$install_docker" = "true" ] && extra_projects+=" and docker"
		[ "$install_nerdctl" = "true" ] && extra_projects+=" and nerdctl"
		info "$kata_project and $extra_projects are now installed"
	fi

	echo -e "\n${warnings}\n"
}

validate_containerd_flavour()
{
	local flavour="${1:-}"
	local flavours_regex='lts|active'

	grep -qE "$flavours_regex" <<< "$flavour" || die "expected flavour to match '$flavours_regex', found '$flavour'"
}

list_versions()
{
	local -r not_installed='<not installed>'

	# The latest available checks will hit the network so inform the
	# user what we are doing in case of network delays.
	info "Getting version details"

	local installed_kata
	installed_kata=$("$kata_shim_v2" --version 2>/dev/null ||\
		echo "$not_installed")

	local installed_containerd
	installed_containerd=$(containerd --version 2>/dev/null ||\
		echo "$not_installed")

	local installed_docker
	installed_docker=$(docker --version 2>/dev/null ||\
		echo "$not_installed")

	local latest_kata
	latest_kata=$(github_get_latest_release "$kata_releases_url" || true)
	[ -z "$latest_kata" ] && \
		die "cannot determine latest version of $project"

	local latest_containerd
	latest_containerd=$(github_get_latest_release "$containerd_releases_url" || true)
	[ -z "$latest_containerd" ] && \
		die "cannot determine latest version of $containerd_project"

	local latest_docker
	latest_docker=$(github_get_latest_release "$docker_releases_url" || true)
	[ -z "$latest_docker" ] && \
		die "cannot determine latest version of $docker_project"

	info "$kata_project: installed version: $installed_kata"
	info "$kata_project: latest version: $latest_kata"

	echo

	info "$containerd_project: installed version: $installed_containerd"
	info "$containerd_project: latest version: $latest_containerd"

	echo

	info "$docker_project: installed version: $installed_docker"
	info "$docker_project: latest version: $latest_docker"
}

# Returns the full path to the specified hypervisors
# configuration file in the specified directory.
#
# Note that the hypervisor "name" argument is not just a literal
# hypervisor name/alias, it is the string that matches the "*" glob
# in the pattern, "configuration-*.toml".
#
# If the specified name is "$default_value",
# use the default hypervisor configuration file name.
#
# If the specified name is not found, return the empty string.
get_hypervisor_config_file()
{
        local name="${1:-}"
	[ -z "$name" ] && die "need name"

        local dir="${2:-}"
	[ -z "$dir" ] && die "need config directory"

	# Expand the default value to the name of the the default hypervisor
	# config file.
	if [ "$name" = "$default_value" ]
	then
		local file
		file="${dir}/${kata_config_file_name}"
		name=$(readlink -e "$file")

		echo "$name" && return 0
	fi

	local -a cfg_files=()

	mapfile -t cfg_files < <(get_hypervisor_config_file_names "$dir")

	local cfg
	for cfg in ${cfg_files[*]}
	do
		local cfg_name
		cfg_name="${cfg#"${dir}/"}"

		local -a possible_names=()

		# Allow a file fragment name or a full name
		# to be specified.
		possible_names+=("configuration-${name}.toml")
		possible_names+=("configuration${name}.toml")
		possible_names+=("${name}.toml")
		possible_names+=("${name}")

		local possible

		for possible in "${possible_names[@]}"
		do
			[ "$possible" = "$cfg_name" ] && echo "$cfg" && return 0
		done
	done

	# Failed to find config file, but don't fail in case it's a
	# local only name.
	echo
}

# Return the name of the config file specified by the name fragment,
# or the empty string if no matching file found.
get_local_cfg_file()
{
        local name="${1:-}"
	[ -z "$name" ] && die "need name"

	local dir="$local_config_dir"

	get_hypervisor_config_file "$name" "$dir"
}

# Return the full path to the pristine config file specified
# by the name fragment.
get_pristine_config_file()
{
        local name="${1:-}"
	[ -z "$name" ] && die "need name"

	local dir="${default_config_dir}"

	local result
	result=$(get_hypervisor_config_file "$name" "$dir")

	echo "$result"
}

# Returns a list of available hypervisor configuration files
# (full paths) for the specified directory.
get_hypervisor_config_file_names()
{
	local dir="${1:-}"
	[ -z "$dir" ] && die "need directory"

	# Note that we do not check for a trailing dash to also match
	# the default config file ("configuration.toml")
	echo "${dir}"/configuration*\.toml
}

# Determine the default hypervisor by looking at sym-link in the
# specified config directory.
get_default_hypervisor_by_dir()
{
	local dir="${1:-}"
	[ -z "$dir" ] && die "need directory"

	local -a cfg_files=()

	mapfile -t cfg_files < <(get_hypervisor_config_file_names "$dir")

	local cfg

	local cfg_type="$pristine_config_type"

	local default_cfg=

	# First, establish what the current default is by resolving
	# the $kata_config_file_name sym link.
	for cfg in ${cfg_files[*]}
	do
		if grep -q "/${kata_config_file_name}$" <<< "$cfg" && [ -h "$cfg" ]
		then
			default_cfg=$(readlink -e "$cfg")
			default_cfg="${default_cfg#"${dir}/"}"
			default_cfg="${default_cfg#configuration-}"
			default_cfg="${default_cfg%.toml}"

			break
		fi
	done

	echo "$default_cfg"
}

# Determine the default hypervisor by looking at sym-link in the
# packaged config directory.
get_default_packaged_hypervisor()
{
	local dir="$default_config_dir"

	local hypervisor
	hypervisor=$(get_default_hypervisor_by_dir "$dir" || true)

	echo "$hypervisor"
}

# Return a list of pristine hypervisor configs, one per line.
# Each line is a tab separated list of fields:
#
# field 1: hypervisor config name.
# field 2: $default_value if this is the default hypervisor,
#          otherwise an empty string.
# field 3: hypervisor config type ($pristine_config_type).
# field 4: hypervisor configs runtime type ($kata_runtime_language).
list_hypervisor_config_file_details_by_dir()
{
	local dir="${1:-}"
	[ -n "$dir" ] || die "need directory"

	local dir_type="${2:-}"
	[ -n "$dir_type" ] || die "need directory type"

	# Ignore a non-existent directory
	[ -d "$dir" ] || return 0

	# First, establish what the current default hypervisor is.
	local default_cfg
	default_cfg=$(get_default_hypervisor_by_dir "$dir" || true)

	local -a cfg_files=()

	mapfile -t cfg_files < <(get_hypervisor_config_file_names "$dir")

	local cfg

	local cfg_type="$pristine_config_type"

	for cfg in ${cfg_files[*]}
	do
		# Ignore the sym-link
		grep -q "/${kata_config_file_name}$" <<< "$cfg" && [ -h "$cfg" ] && continue

		local cfg_name
		cfg_name="${cfg#"${dir}/"}"
		cfg_name="${cfg_name#configuration-}"
		cfg_name="${cfg_name%.toml}"

		local cfg_default_value='-'

		[ "$cfg_name" = "$default_cfg" ] && cfg_default_value="$default_value"

		printf "%s\t%s\t%s\t%s\n" \
			"$cfg_name" \
			"$cfg_default_value" \
			"$cfg_type" \
			"$dir_type"
	done | sort -u
}

# Display a list of packaged hypervisor config file name fragments,
# one line per config file. Each line is a set of fields.
#
# See list_hypervisor_config_file_details_by_dir() for the fields
# displayed.
list_packaged_hypervisor_config_names()
{
	ensure_kata_already_installed

	local golang_cfgs
	golang_cfgs=$(list_hypervisor_config_file_details_by_dir \
		"$kata_config_dir_golang" \
		"$kata_runtime_language")

	echo "${golang_cfgs}"
}

# Display a list of local hypervisor config file name fragments,
# one line per config file. Each line is a set of fields.
#
# See list_hypervisor_config_file_details_by_dir() for the fields
# displayed.
list_local_hypervisor_config_names()
{
	ensure_kata_already_installed

	local golang_cfgs
	golang_cfgs=$(list_hypervisor_config_file_details_by_dir \
		"$local_config_dir" \
		"$kata_runtime_language")

	echo "${golang_cfgs}"
}

# Change the configured hypervisor to the one specified.
#
# This function creates a local Kata configuration file if one does
# not exist and creates a symbolic link to it.
set_kata_config_file()
{
	ensure_kata_already_installed

	local force="${1:-}"
	[ -z "$force" ] && die "no force value"

	local hypervisor="${2:-}"
	[ -z "$hypervisor" ] && die "no hypervisor value"

	local hypervisor_cfg_file
	hypervisor_cfg_file=$(get_pristine_config_file "$hypervisor")

	sudo mkdir -p "$local_config_dir"

	# The name of the local config file
	local local_cfg_file

	# The name of the local kata config file sym-link
	local local_kata_cfg_symlink="$kata_config_file"

	if [ -n "$hypervisor_cfg_file" ]
	then
		# A pristine hypervisor config file exists

		local hypervisor_cfg_file_name
		hypervisor_cfg_file_name=$(basename "$hypervisor_cfg_file")

		sudo mkdir -p "$local_config_dir"

		local_cfg_file="${local_config_dir}/${hypervisor_cfg_file_name}"

		if [ -e "$local_cfg_file" ]
		then
			if [ -n "$(diff "$local_cfg_file" "$hypervisor_cfg_file")" ]
			then
				[ "$force" = 'false' ] && \
					die "existing hypervisor config file '$local_cfg_file' differs from pristine version: '$hypervisor_cfg_file'"
			fi
		fi

		# First, install a copy of the config file to the local config dir.
		sudo install \
			-o root \
			-g root \
			-m "$cfg_file_install_perms" \
			"$hypervisor_cfg_file" \
			"$local_cfg_file"

	else
		# There is no pristine config file, so look for a
		# local-only config file.
		local_cfg_file=$(get_local_cfg_file "$hypervisor" || true)
		[ -z "$local_cfg_file" ] && \
			die "no packaged or local hypervisor config file found for '$hypervisor'"
	fi

	# Next, create the standard sym-link, pointing to the
	# requested hypervisor-specific config file, taking care to
	# fail if the default config file is not a sym-link already.
	if [ -e "$local_kata_cfg_symlink" ]
	then
		if [ ! -L "$local_kata_cfg_symlink" ]
		then
			# Don't do this, even if force is in operation
			# as we don't know what the contents of the
			# file are.
			die "not overwriting non-sym-link config file: '$local_kata_cfg_symlink'"
		fi

		if [ "$force" = false ]
		then
			# Back up the existing version of the file
			local now
			now=$(date '+%Y-%m-%d.%H-%M-%S.%N')

			local backup_name
			backup_name="${local_kata_cfg_symlink}.${now}.saved"

			sudo install -o root -g root \
				-m "$cfg_file_install_perms" \
				"$local_kata_cfg_symlink" \
				"$backup_name"
		fi
	fi

	local default_hypervisor
	default_hypervisor=$(get_default_packaged_hypervisor || true)

	# We can now safely force install the sym-link.
	sudo ln -sf "$local_cfg_file" "$local_kata_cfg_symlink"

	local hypervisor_descr="'$hypervisor'"

	[ "$hypervisor" = "$default_value" ] || [ "$hypervisor" = "$default_hypervisor" ] && \
		hypervisor_descr="'$default_hypervisor' ($default_value)"

	info "Set config to $hypervisor_descr"
}

handle_args()
{
	local cleanup="true"
	local force="false"
	local skip_containerd="false"
	local disable_test="false"
	local only_run_test="false"
	local enable_debug="false"
	local install_docker="false"
	local install_nerdctl="false"
	local list_versions='false'
	local hypervisor="$default_value"
	local switch_to_hypervisor=''
	local list_available_pristine_hypervisor_configs='false'
	local list_available_local_hypervisor_configs='false'

	local opt

	local kata_version=""
	local containerd_flavour="lts"
	local kata_tarball=""

	while getopts "c:dDefhH:k:K:lLNorS:tT" opt "$@"
	do
		case "$opt" in
			c) containerd_flavour="$OPTARG" ;;
			d) enable_debug="true" ;;
			D) install_docker="true" ;;
			e) list_available_local_hypervisor_configs='true' ;;
			f) force="true" ;;
			h) usage; exit 0 ;;
			H) hypervisor="$OPTARG" ;;
			k) kata_version="$OPTARG" ;;
			K) kata_tarball="$OPTARG" ;;
			l) list_versions='true' ;;
			L) list_available_pristine_hypervisor_configs='true' ;;
			N) install_nerdctl="true" ;;
			o) skip_containerd="true" ;;
			r) cleanup="false" ;;
			S) switch_to_hypervisor="$OPTARG" ;;
			t) disable_test="true" ;;
			T) only_run_test="true" ;;

			*) die "invalid option: '$opt'" ;;
		esac
	done

	shift $[$OPTIND-1]

	[ "$list_versions" = 'true' ] && list_versions && exit 0

	if [ "$list_available_pristine_hypervisor_configs" = true ]
	then
		list_packaged_hypervisor_config_names
		exit 0
	fi

	if [ "$list_available_local_hypervisor_configs" = true ]
	then
		list_local_hypervisor_config_names
		exit 0
	fi

	if [ -n "$switch_to_hypervisor" ]
	then
		# XXX: If the user is asking to switch hypervisor
		# config, to keep life simpler and to avoid polluting
		# config directories with backup files, we make the assumption
		# that they have already backed up any needed config
		# files.
		#
		# Note that the function below checks that Kata is
		# installed.
		force='true'

		set_kata_config_file \
			"$force" \
			"$switch_to_hypervisor"
		exit 0
	fi

	[ -z "$kata_version" ] && kata_version="${1:-}" || true
	[ -z "$containerd_flavour" ] && containerd_flavour="${2:-}" || true

	validate_containerd_flavour "$containerd_flavour"

	handle_installation \
		"$cleanup" \
		"$force" \
		"$skip_containerd" \
		"$enable_debug" \
		"$disable_test" \
		"$only_run_test" \
		"$kata_version" \
		"$containerd_flavour" \
		"$install_docker" \
		"$install_nerdctl" \
		"$kata_tarball" \
		"$hypervisor"
}

main()
{
	handle_args "$@"
}

main "$@"
