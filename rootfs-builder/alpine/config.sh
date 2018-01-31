# This is a configuration file add extra variables to
# be used by build_rootfs() from rootfs_lib.sh the variables will be
# loaded just before call the function.

# Here there are a couple of variables you may need.
# Remove them or add more

# alpine version
OS_VERSION=${OS_VERSION:-v3.7}

# Essential base packages
BASE_PACKAGES="alpine-base"

# Alpine mirror to use
# See a list of mirrors at http://nl.alpinelinux.org/alpine/MIRRORS.txt
MIRROR=http://dl-5.alpinelinux.org/alpine

# Default Architecture
ARCH=${ARCH:-x86_64}

# Mandatory Packages that must be installed
#  - iptables: Need by Kata agent
PACKAGES="iptables"
