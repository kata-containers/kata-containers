#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Hardware diagnostics only. Kubernetes and Kata installation use upstream CI.
set -euo pipefail

hardware() {
    : "${ARCH:?ARCH must be supplied by the lab controller}"
    test "$(uname -s)" = Linux
    case "${ARCH}:$(uname -m)" in
        amd64:x86_64|arm64:aarch64) ;;
        *) echo "Target architecture does not match the machine" >&2; exit 1 ;;
    esac
    test -c /dev/kvm || { echo 'Missing /dev/kvm: enable virtualization/nesting on this machine' >&2; exit 1; }
    # Opening the device alone is insufficient: check the KVM API and create a VM.
    python3 - <<'PY'
import fcntl, os
fd = os.open('/dev/kvm', os.O_RDWR | os.O_CLOEXEC)
try:
    assert fcntl.ioctl(fd, 0xAE00, 0) == 12, 'Unexpected KVM API version'
    vm = fcntl.ioctl(fd, 0xAE01, 0)
    os.close(vm)
finally:
    os.close(fd)
PY
    command -v systemctl >/dev/null
    printf 'Host: '; uname -a
    systemd-detect-virt || true
    lsmod
}

probe() {
    hardware
    local qemu
    case "${ARCH}" in
        arm64) qemu=/opt/kata/bin/qemu-system-aarch64 ;;
        amd64) qemu=/opt/kata/bin/qemu-system-x86_64 ;;
    esac
    "${qemu}" --version
    local status=0
    if test "${ARCH}" = arm64; then
        timeout 5 "${qemu}" -machine virt,accel=kvm,gic-version=host -cpu host \
            -m 128 -smp 1 -S -nodefaults -nographic || status=$?
    else
        timeout 5 "${qemu}" -machine q35,accel=kvm -cpu host \
            -m 128 -smp 1 -S -nodefaults -nographic || status=$?
    fi
    if test "${status}" = 124; then
        echo 'QEMU initialized with KVM and remained running until the probe timeout'
    else
        echo "QEMU KVM initialization returned ${status}" >&2
        return 1
    fi
    # Exercise vCPUs and boot the installed image read-only, with serial output.
    # This cannot establish agent readiness and does not change artifacts.
    if test "${ARCH}" = arm64; then
        local firmware
        for firmware in firmware direct; do
            local -a extra=()
            if test "${firmware}" = firmware; then
                extra=(-bios /opt/kata/share/aavmf/AAVMF_CODE.fd)
            fi
            echo "ARM kernel boot diagnostic: ${firmware} (read-only root disk)"
            status=0
            timeout 20 "${qemu}" -machine virt,accel=kvm,gic-version=host -cpu host \
                -m 2048 -smp 1 -nodefaults -nographic -no-reboot -monitor none -serial stdio \
                -kernel /opt/kata/share/kata-containers/vmlinux.container \
                -drive file=/opt/kata/share/kata-containers/kata-containers.img,format=raw,if=none,id=root,readonly=on \
                -device virtio-blk-pci,drive=root \
                -append 'console=ttyAMA0 earlycon=pl011,0x9000000 root=/dev/vda1 ro rootfstype=ext4 systemd.unit=kata-containers.target selinux=0 panic=1' \
                "${extra[@]}" || status=$?
            echo "Boot diagnostic exited with ${status} (124 means timeout)"
        done
    fi
}

case "${1:-}" in
    library) ;;
    doctor) hardware ;;
    probe) probe ;;
    *) echo "Expected doctor or probe" >&2; exit 2 ;;
esac
