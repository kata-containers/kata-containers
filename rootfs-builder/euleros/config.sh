OS_NAME="EulerOS"

OS_VERSION=${OS_VERSION:-2.2}

BASE_URL="http://developer.huawei.com/ict/site-euleros/euleros/repo/yum/${OS_VERSION}/os/${ARCH}/"

GPG_KEY_FILE="RPM-GPG-KEY-EulerOS"

PACKAGES="iptables"

#Optional packages:
# systemd: An init system that will start kata-agent if kata-agent
#          itself is not configured as init process.
[ "$AGENT_INIT" == "no" ] && PACKAGES+=" systemd" || true
