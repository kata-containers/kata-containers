#!/run/kata-extensions/devkit/bin/busybox sh
# Thin apk wrapper: set up the overlay/chroot and run Alpine's apk from the
# minimal Alpine rootfs natively.  Installed under usr/bin/devkit-apk.
#
# Example: `devkit-apk add htop` installs into the writable overlay at runtime.
DEVKIT=/run/kata-extensions/devkit
. "${DEVKIT}/usr/bin/devkit-init"

devkit_chroot_exec /sbin/apk "$@"
