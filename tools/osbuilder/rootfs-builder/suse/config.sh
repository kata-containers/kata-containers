#
# Copyright (c) 2018 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0

# May also be "Tumbleweed"
OS_DISTRO="Leap"

# Leave this empty for distro "Tumbleweed"
OS_VERSION=${OS_VERSION:-15.0}

OS_IDENTIFIER="$OS_DISTRO${OS_VERSION:+:$OS_VERSION}"

# Extra packages to install in the rootfs
PACKAGES="systemd iptables libudev1"

#  http or https
REPO_TRANSPORT="https"

# Can specify an alternative domain
REPO_DOMAIN="download.opensuse.org"

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=systemd
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
ARCH_EXCLUDE_LIST=()

###############################################################################
#
# NOTE: you probably dont need to edit things below this
#

SUSE_URL_BASE="${REPO_TRANSPORT}://${REPO_DOMAIN}"
SUSE_PATH_OSS="/distribution/${OS_DISTRO,,}/$OS_VERSION/repo/oss"
SUSE_PATH_UPDATE="/update/${OS_DISTRO,,}/$OS_VERSION/oss"

arch="$(uname -m)"
case "$arch" in
	x86_64)
		REPO_URL_PORT=""
		;;
	ppc|ppc64le)
		REPO_URL_PORT="/ports/ppc"
		;;
	aarch64)
		REPO_URL_PORT="/ports/aarch64"
		;;
	*)
		die "Unsupported architecture: $arch"
		;;
esac
SUSE_FULLURL_OSS="${SUSE_URL_BASE}${REPO_URL_PORT}${SUSE_PATH_OSS}"
SUSE_FULLURL_UPDATE="${SUSE_URL_BASE}${SUSE_PATH_UPDATE}"

if [ -z "${REPO_URL:-}" ]; then
	REPO_URL="$SUSE_FULLURL_OSS"
fi
