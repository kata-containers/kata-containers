# This is a configuration file add extra variables to
# be used by build_rootfs() from rootfs_lib.sh the variables will be
# loaded just before call the function.

# Here there are a couple of variables you may need.
# Remove them or add more 

# EulerOS Version
OS_VERSION=${OS_VERSION:-2.2}

#Mandatory Packages that must be installed
# iptables: Need by Kata agent
PACKAGES="iptables"

#Optional packages:
# systemd: An init system that will start kata-agent if kata-agent
#          itself is not configured as init process.
[ "$AGENT_INIT" == "no" ] && PACKAGES+=" systemd" || true
