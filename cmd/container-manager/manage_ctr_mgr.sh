#!/bin/bash -e
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

DOCKER_BIN=docker
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
SCRIPT_NAME=${0##*/}
source "${SCRIPT_PATH}/../../.ci/lib.sh"
source /etc/os-release || source /usr/lib/os-release

force=false
ctr_manager=""
subcommand=""
runtime=""
tag=""

usage(){
	cat << EOF
This script helps you install the correct version of docker
to use with Clear Containers.
WARNING: Using this tool with -f flag, will overwrite any docker configuration that you may
have modified.
Usage: $SCRIPT_NAME [docker] [configure|info|install|remove] <options>
Options:
	-f                         : Force action. It will replace any installation
                                     or configuration that you may have.
	-h                         : Help, show this information.
	-r <runtime>               : Supported runtimes: runc and kata-runtime.
	-s <storage_driver>        : Supported storage driver: overlay2(default), devicemapper, etc.
	-t <tag>                   : Tags supported: swarm, latest. If you do not specify
                                     a tag, the script will use latest as default.
                                     With this tag you can install the correct version
                                     of docker that has CC compatibility with swarm.
Example:
	./$SCRIPT_NAME docker install -t swarm -f
EOF
}

die(){
	msg="$*"
	echo "$SCRIPT_NAME - ERROR: $msg" >&2
	exit 1
}

warning(){
	msg="$*"
	echo "$SCRIPT_NAME - WARNING: $msg" >&2
}

message(){
	msg="$*"
	echo "$SCRIPT_NAME - INFO: $msg" >&2
}

log_message(){
	message="$1"
	logger -s -t "$(basename $0)" "$message"
}

parse_subcommand_options(){
	while getopts ":fr:s:t:" opt; do
		case $opt in
			f)
				force=true
				;;
			r)
				runtime="${OPTARG}"
				;;
			s)
				storage_driver="${OPTARG}"
				;;
			t)
				tag="${OPTARG}"
				;;
			\?)
				echo "Invalid option: -${OPTARG}" >&2
				usage
				exit 1
		esac
	done
}

# This function handles the installation of the required docker version.
install_docker(){
	# Get system architecture
	arch=$(go env GOARCH)
	# Check if docker is present in the system
	if [ "$(info_docker)" ] && [ ${force} == false ]; then
		die "Docker is already installed. Please use -f flag to force new installation"
	elif [ "$(info_docker)" ] && [ ${force} == true ]; then
		remove_docker
	fi

	if [ -z "$tag" ] || [ "$tag" == "latest" ] ; then
		# If no tag is recevied, install latest compatible version
		docker_version=$(get_version "externals.docker.version")
		log_message "Installing docker $docker_version"
		docker_version=${docker_version/v}
		docker_version=${docker_version/-*}
		pkg_name="docker-ce"
		if [ "$ID" == "ubuntu" ]; then
			sudo -E apt-get -y install apt-transport-https ca-certificates software-properties-common
			repo_url="https://download.docker.com/linux/ubuntu"
			curl -fsSL "${repo_url}/gpg" | sudo apt-key add -
			sudo -E add-apt-repository "deb [arch=${arch}] ${repo_url} $(lsb_release -cs) stable"
			sudo -E apt-get update
			docker_version_full=$(apt-cache madison $pkg_name | grep "$docker_version" | awk '{print $3}' | head -1)
			sudo -E apt-get -y install "${pkg_name}=${docker_version_full}"
		elif [ "$ID" == "fedora" ]; then
			repo_url="https://download.docker.com/linux/fedora/docker-ce.repo"
			sudo -E dnf -y install dnf-plugins-core
			sudo -E dnf config-manager --add-repo "$repo_url"
			if [ "$VERSION_ID" -ge "30" ]; then
				warning "This step will be removed once  https://github.com/kata-containers/tests/issues/1954 is solved"
				sudo sed -i 's/$releasever/28/' /etc/yum.repos.d/docker-ce.repo
				sudo -E dnf config-manager --set-enabled docker-ce-stable
				sudo -E dnf makecache
				sudo -E dnf install -y docker-ce-18.06.3.ce-3.fc28
			else
				sudo -E dnf makecache
				docker_version_full=$(dnf --showduplicate list "$pkg_name" | grep "$docker_version" | awk '{print $2}' | tail -1)
				sudo -E dnf -y install "${pkg_name}-${docker_version_full}"
			fi
		elif [ "$ID" == "centos" ] || [ "$ID" == "rhel" ]; then
			sudo -E yum install -y yum-utils
			repo_url="https://download.docker.com/linux/centos/docker-ce.repo"
			sudo yum-config-manager --add-repo "$repo_url"
			sudo yum makecache
			docker_version_full=$(yum --showduplicate list "$pkg_name" | \
				grep "$docker_version" | awk '{print $2}' | tail -1 | cut -d':' -f2)
			sudo -E yum -y install "${pkg_name}-${docker_version_full}"
		elif [ "$ID" == "debian" ]; then
			sudo -E apt-get -y install apt-transport-https ca-certificates software-properties-common
			curl -sL https://download.docker.com/linux/debian/gpg | sudo apt-key add -
			arch=$(dpkg --print-architecture)
			sudo -E add-apt-repository "deb [arch=${arch}] https://download.docker.com/linux/debian $(lsb_release -cs) stable"
			sudo -E apt-get update
			docker_version_full=$(apt-cache madison $pkg_name | grep "$docker_version" | awk '{print $3}' | head -1)
			sudo -E apt-get -y install "${pkg_name}=${docker_version_full}"
		elif [[ "$ID" =~ ^opensuse.*$ ]] || [ "$ID" == "sles" ]; then
			sudo zypper removelock docker
			sudo zypper -n  install 'docker<19.03'
			sudo zypper addlock docker
		fi
	elif [ "$tag" == "swarm" ]; then
		# If tag is swarm, install docker 1.12.1
		log_message "Installing docker $docker_swarm_version"
		pkg_name="docker-engine"
		if [ "$ID" == "ubuntu" ] || [ "$ID" == "debian" ]; then
			# We stick to the xenial repo, since it is the only one that
			# provides docker 1.12.1
			repo_url="https://apt.dockerproject.org"
			sudo -E apt-get -y install apt-transport-https ca-certificates
			curl -fsSL "${repo_url}/gpg" | sudo apt-key add -
			sudo -E add-apt-repository "deb [arch=${arch}] ${repo_url}/repo ubuntu-xenial main"
			sudo -E apt-get update
			docker_version_full=$(apt-cache show docker-engine | grep "^Version: $docker_swarm_version" | awk '{print $2}' | head -1)
			sudo -E apt-get -y install --allow-downgrades "${pkg_name}=${docker_version_full}"
		elif [ "$ID" == "fedora" ]; then
			# We stick to the Fedora 24 repo, since it is the only one that
			# provides docker 1.12.1
			repo_url="https://yum.dockerproject.org"
			fedora24_repo="${repo_url}/repo/main/fedora/24"
			gpg_key="gpg"
			sudo -E dnf -y install dnf-plugins-core
			sudo -E dnf config-manager --add-repo  "${fedora24_repo}"
			curl -O "${repo_url}/${gpg_key}"
			sudo rpm --import "./${gpg_key}"
			rm "./${gpg_key}"
			sudo -E dnf makecache
			docker_version_full=$(dnf --showduplicate list "$pkg_name" | grep "$docker_swarm_version" | awk '{print $2}' | tail -1)
			sudo -E dnf -y install "${pkg_name}-${docker_version_full}"
		elif [ "$ID" == "centos" ]; then
			repo_url="https://yum.dockerproject.org"
			centos7_repo="${repo_url}/repo/main/centos/7"
			gpg_key="gpg"
			sudo -E yum -y install yum-utils
			sudo -E yum config-manager --add-repo  "${centos7_repo}"
			curl -O "${repo_url}/${gpg_key}"
			sudo rpm --import "./${gpg_key}"
			rm "./${gpg_key}"
			sudo -E yum makecache
			docker_version_full=$(yum --showduplicate list "$pkg_name" | grep "$docker_swarm_version" | awk '{print $2}' | tail -1)
			sudo -E yum -y install "${pkg_name}-${docker_version_full}"
		fi
	else
		# If tag received is invalid, then return an error message
		die "Unrecognized tag. Tag supported is: swarm"
	fi
	sudo systemctl restart docker
	sudo gpasswd -a ${USER} docker
	sudo chmod g+rw /var/run/docker.sock
}

# This function removes the installed docker package.
remove_docker(){
	pkg_name=$(get_docker_package_name)
	if [ -z "$pkg_name" ]; then
		die "Docker not found in this system"
	else
		sudo systemctl stop docker
		version=$(get_docker_version)
		log_message "Removing package: $pkg_name version: $version"
		if [ "$ID" == "ubuntu" ] || [ "$ID" == "debian" ]; then
			sudo apt -y purge ${pkg_name}
		elif [ "$ID" == "fedora" ]; then
			sudo dnf -y remove ${pkg_name}
		elif [ "$ID" == "centos" ] || [ "$ID" == "rhel" ]; then
			sudo yum -y remove ${pkg_name}
		elif [[ "$ID" =~ ^opensuse.*$ ]] || [ "$ID" == "sles" ]; then
			sudo zypper removelock ${pkg_name}
			sudo zypper -n remove ${pkg_name}
		else
			die "This script doesn't support your Linux distribution"
		fi
	fi
}

get_docker_default_runtime(){
	sudo docker info 2> /dev/null | awk '/Default Runtime/ {print $3}'
}

get_docker_version(){
	sudo docker version | awk '/Engine/{getline; print $2 }'
}

get_docker_package_name(){
	if [ "$ID" == "ubuntu" ] || [ "$ID" == "debian" ]; then
		dpkg --get-selections | awk '/docker/ {print $1}'
	elif [ "$ID" == "fedora" ] || [ "$ID" == "centos" ] || [ "$ID" == "rhel" ] || [[ "$ID" =~ ^opensuse.*$ ]] || [ "$ID" == "sles" ]; then
		rpm -qa | grep docker | grep -v selinux
	else
		die "This script doesn't support your Linux distribution"
	fi
}

# This function gives information about:
# - Installed docker package and version
# - docker default runtime
info_docker(){
	if command -v "$DOCKER_BIN"; then
		message "docker_info: version: $(get_docker_version)"
		message "docker_info: default runtime: $(get_docker_default_runtime)"
		message "docker_info: package name: $(get_docker_package_name)"
	else
		warning "docker is not installed on this system"
		return 1
	fi
}

# Modify docker service using of $docker_options
modify_docker_service(){
	docker_options=$1
	docker_service_dir="/etc/systemd/system/docker.service.d/"
	if [ "$(ls -A $docker_service_dir)" ] && [ ${force} == false ]; then
		die "Found a service configuration file. Please use -f flag to overwrite the service configuration"
	elif [ "$(ls -A $docker_service_dir)" ] && [ ${force} == true ]; then
		rm -rf "${docker_service_dir}/*"
	fi
	echo "Stopping the docker service"
	sudo systemctl stop docker
	dir="/var/lib/docker"
	echo "Removing $dir"
	[ -d "$dir" ] && sudo rm -rf "$dir"
	echo "Changing docker service configuration"
	sudo mkdir -p "$docker_service_dir"
	cat <<EOF | sudo tee "$docker_service_dir/kata-containers.conf"
[Service]
Environment="$docker_http_proxy"
Environment="$docker_https_proxy"
ExecStart=
ExecStart=/usr/bin/dockerd ${docker_options}
EOF
	echo "Reloading unit files and starting docker service"
	sudo systemctl daemon-reload
	sudo systemctl restart docker
}

# This function configures docker to work by default with the
# specified runtime.
configure_docker(){
	[ -z "$runtime" ] && die "please specify a runtime with -r"

	# Default storage driver is overlay2
	[ -z "$storage_driver" ] && storage_driver="overlay2"

	if [ ! "$(info_docker)" ]; then
		die "Docker is not installed. Please install it before configuring the runtime"
	fi

	if [ "$(get_docker_default_runtime)" == "$runtime" ]; then
		message "configure_docker: $runtime is already configured as default runtime"
	else
		log_message "configure_docker: configuring $runtime as default docker runtime"
		# Check if the system has set http[s] proxy
		if [ -n "$http_proxy" ] && [ -n "$https_proxy" ] ;then
			docker_http_proxy="HTTP_PROXY=$http_proxy"
			docker_https_proxy="HTTPS_PROXY=$https_proxy"
		fi

		if [ "$tag" == "swarm" ] ; then
			default_runtime=$runtime
		else
			default_runtime="runc"
		fi

		if [ "$runtime" == "kata-runtime" ]  ; then
			# Try to find kata-runtime in $PATH, if it is not present
			# then the default location will be /usr/local/bin/kata-runtime
			if [ "$ID" == "fedora" ] || [ "$ID" == "centos" ]; then
				kata_runtime_bin="$(whereis $runtime | cut -f2 -d':' | tr -d "[:space:]")" || \
					die "$runtime cannot be found in $PATH, please make sure it is installed"
			else
				kata_runtime_bin="$(which $runtime)" || \
					die "$runtime cannot be found in $PATH, please make sure it is installed"
			fi
			docker_options="-D --add-runtime $runtime=$kata_runtime_bin --default-runtime=$default_runtime --storage-driver=$storage_driver"
			modify_docker_service "$docker_options"
		elif [ "$runtime" == "runc" ]  ; then
			docker_options="-D --storage-driver=$storage_driver"
			modify_docker_service "$docker_options"
		else
			die "configure_docker: runtime $runtime not supported"
		fi
	fi
}

main(){
	# Check if the script is run without arguments
	[ "$#" -eq 0 ] && usage && exit 1

	# Parse Usage options:
	while getopts ":h" opt; do
		case ${opt} in
			h )
				usage
				exit 0
				;;
			\? )
				echo "Invalid Option: -$OPTARG" 1>&2
				usage
				exit 1
				;;
		esac
	done
	shift $((OPTIND -1))

	ctr_manager=$1; shift
	case "$ctr_manager" in
	# Parse options
		docker)
			subcommand=$1; shift
			parse_subcommand_options "$@"
			;;
		*)
			warning "container manager \"$ctr_manager\" is not supported."
			usage
			exit 1
	esac

	shift "$((OPTIND - 1))"

	case "$subcommand" in
		configure )
			configure_docker
			;;

		info )
			info_docker
		;;

		install )
			install_docker
			;;

		remove )
			remove_docker
			;;

		*)
			echo "Invalid subcommand: \"$subcommand\""
			usage
			exit 1

	esac
	echo "Script finished"
}

main "$@"
