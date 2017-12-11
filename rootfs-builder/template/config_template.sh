# This is a configuration file add extra variables to
# be used by build_rootfs() from rootfs_lib.sh the variables will be
# loaded just before call the function.

# Here there are a couple of variables you may need.
# Remove them or add more 

#Use it rootfs is based in a system has different versions
OS_VERSION=${OS_VERSION:-DEFAULT_VERSION}

#Mandatory Packages that must be installed
# systemd: An init system that will start kata-agent
# iptables: Need by Kata agent
# udevlib.so: Need by Kata agent
PACKAGES="systemd iptables udevlib.so"
