# Builder stage
FROM ubuntu:22.04 AS builder

# CI_COMMIT_TAG will be passed as build arg (e.g., 3.18.0-dd.202526)
ARG CI_COMMIT_TAG

# Install packages
RUN apt-get update && \
    apt-get install -y curl wget unzip xz-utils zstd ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create directory structure
RUN mkdir -p /kata-build/opt/kata/bin \
             /kata-build/opt/kata/conf/qemu/config.d \
             /kata-build/opt/kata/libexec \
             /kata-build/opt/kata/share/kata-containers \
             /kata-build/etc/containerd/config.d

# Download and extract kata package
# Extract version from tag and construct URL
ARG TARGETARCH
RUN KATA_VERSION=${CI_COMMIT_TAG%%-dd.*} && \
    echo "Downloading kata ${KATA_VERSION} from tag ${CI_COMMIT_TAG}" && \
    wget -q "https://github.com/kata-containers/kata-containers/releases/download/${KATA_VERSION}/kata-static-${KATA_VERSION}-${TARGETARCH}.tar.zst" -O /tmp/kata.tar.zst && \
    tar --zstd -xf /tmp/kata.tar.zst -C /kata-build --wildcards \
        './opt/kata/bin/qemu-system-*' \
        './opt/kata/libexec/virtiofsd' \
        './opt/kata/lib/kata-qemu/libfdt.a' \
        './opt/kata/lib/kata-qemu/pkgconfig/libfdt.pc' \
        './opt/kata/include/fdt.h' \
        './opt/kata/include/libfdt.h' \
        './opt/kata/include/libfdt_env.h' \
        './opt/kata/share/kata-qemu/qemu/bios-256k.bin' \
        './opt/kata/share/kata-qemu/qemu/bios-microvm.bin' \
        './opt/kata/share/kata-qemu/qemu/bios.bin' \
        './opt/kata/share/kata-qemu/qemu/edk2-i386-code.fd' \
        './opt/kata/share/kata-qemu/qemu/edk2-i386-secure-code.fd' \
        './opt/kata/share/kata-qemu/qemu/edk2-i386-vars.fd' \
        './opt/kata/share/kata-qemu/qemu/edk2-x86_64-code.fd' \
        './opt/kata/share/kata-qemu/qemu/edk2-x86_64-secure-code.fd' \
        './opt/kata/share/kata-qemu/qemu/edk2-licenses.txt' \
        './opt/kata/share/kata-qemu/qemu/efi-virtio.rom' \
        './opt/kata/share/kata-qemu/qemu/firmware/50-edk2-i386-secure.json' \
        './opt/kata/share/kata-qemu/qemu/firmware/50-edk2-x86_64-secure.json' \
        './opt/kata/share/kata-qemu/qemu/firmware/60-edk2-i386.json' \
        './opt/kata/share/kata-qemu/qemu/firmware/60-edk2-x86_64.json' \
        './opt/kata/share/kata-qemu/qemu/kvmvapic.bin' \
        './opt/kata/share/kata-qemu/qemu/linuxboot.bin' \
        './opt/kata/share/kata-qemu/qemu/linuxboot_dma.bin' \
        './opt/kata/share/kata-qemu/qemu/multiboot_dma.bin' \
        './opt/kata/share/kata-qemu/qemu/pvh.bin' \
        './opt/kata/share/kata-qemu/qemu/qboot.rom' \
        './opt/kata/share/kata-qemu/qemu/qemu-nsis.bmp' \
        './opt/kata/share/defaults/kata-containers/configuration-qemu.toml' \
        './opt/kata/share/kata-containers/kata-ubuntu-noble.image' && \
    mv /kata-build/opt/kata/share/kata-containers/kata-ubuntu-noble.image /kata-build/opt/kata/share/kata-containers/kata-containers-datadog.img && \
    rm -f /tmp/kata.tar.xz

# Copy CI-built kernel for the target architecture and rename to datadog convention
COPY tools/packaging/kernel/vmlinux-${TARGETARCH} /kata-build/opt/kata/share/kata-containers/vmlinux-datadog

# Copy CI-built shim binary
# This should be built by the CI pipeline and placed in the expected location
COPY src/runtime/containerd-shim-kata-v2-${TARGETARCH} /kata-build/opt/kata/bin/containerd-shim-kata-v2
RUN chmod +x /kata-build/opt/kata/bin/containerd-shim-kata-v2

# Create symlinks for kata-containers.img and vmlinux.container
RUN ln -s /opt/kata/share/kata-containers/kata-containers-datadog.img /kata-build/opt/kata/share/kata-containers/kata-containers.img && \
    ln -s /opt/kata/share/kata-containers/vmlinux-datadog /kata-build/opt/kata/share/kata-containers/vmlinux.container

# Link configuration
RUN ln /kata-build/opt/kata/share/defaults/kata-containers/configuration-qemu.toml /kata-build/opt/kata/conf/qemu/configuration-qemu.toml

# Copy override configuration
COPY docker/qemu/config.d/10-override.toml /kata-build/opt/kata/conf/qemu/config.d/10-override.toml

# Copy containerd runtime dropin
COPY docker/containerd/config.d/20-kata-runtime.toml /kata-build/etc/containerd/config.d/20-kata-runtime.toml

# Final scratch-based data volume
FROM scratch
COPY --from=builder /kata-build/opt/kata /opt/kata
COPY --from=builder /kata-build/etc/containerd /etc/containerd
