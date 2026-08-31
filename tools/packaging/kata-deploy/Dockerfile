# Copyright Intel Corporation, 2022 IBM Corp.
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

#### Prepare kata artifact tarballs
# Individual component tarballs are stored compressed in the image.
# The installer extracts only those required for the enabled runtime classes.
FROM alpine:3.22 AS artifact-stage

ARG KATA_ARTIFACTS_DIR=tools/packaging/kata-deploy/kata-artifacts
ARG DESTINATION=/opt/kata-artifacts

SHELL ["/bin/ash", "-eo", "pipefail", "-c"]

RUN apk add --no-cache zstd

COPY ${KATA_ARTIFACTS_DIR}/kata-static-*.tar.zst ${DESTINATION}/tarballs/
COPY ${KATA_ARTIFACTS_DIR}/kata-deploy-static-*.tar.zst ${DESTINATION}/tarballs/
COPY tools/packaging/kata-deploy/shim-components.json ${DESTINATION}/
COPY versions.yaml ${DESTINATION}/

RUN \
	mkdir -p /opt/prebuilt && \
	zstd -dc "${DESTINATION}/tarballs/kata-deploy-static-kata-deploy-binary.tar.zst" | tar -xf - -C /opt/prebuilt && \
	zstd -dc "${DESTINATION}/tarballs/kata-deploy-static-nydus-snapshotter-for-coco-guest-pull.tar.zst" | tar -xf - -C /opt/prebuilt

#### Prepare runtime libraries
# The kata-deploy binary is dynamically linked on ppc64le and s390x.
FROM debian:trixie-slim AS runtime-assembler

ARG DESTINATION=/opt/kata-artifacts

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Copy the prebuilt binary to analyze its dependencies
COPY --from=artifact-stage /opt/prebuilt/usr/bin/kata-deploy /tmp/kata-deploy

# Create output directories
RUN mkdir -p /output/lib /output/lib64

# Use ldd to find and copy all required libraries for the kata-deploy binary.
RUN \
	HOST_ARCH="$(uname -m)"; \
	echo "Preparing runtime dependencies for ${HOST_ARCH}"; \
	case "${HOST_ARCH}" in \
		"ppc64le"|"s390x") \
			echo "Using glibc - copying libraries based on ldd output"; \
			\
			# Show what the binary needs \
			echo "Libraries needed by kata-deploy:"; \
			ldd /tmp/kata-deploy || echo "ldd failed"; \
			\
			# Extract and copy all shared library paths \
			ldd /tmp/kata-deploy 2>/dev/null | grep "=>" | awk '{print $3}' | sort -u | while read -r lib; do \
				if [ -n "${lib}" ] && [ -f "${lib}" ]; then \
					dest_dir="/output$(dirname "${lib}")"; \
					mkdir -p "${dest_dir}"; \
					cp -Ln "${lib}" "${dest_dir}/" 2>/dev/null || true; \
					echo "  Copied lib: ${lib}"; \
				fi; \
			done; \
			\
			# Copy the dynamic linker - it's at /lib/ld64.so.1 (not /lib64/) \
			echo "Copying dynamic linker:"; \
			mkdir -p /output/lib; \
			cp -Ln /lib/ld64.so* /output/lib/ 2>/dev/null || true; \
			cp -Ln /lib64/ld64.so* /output/lib64/ 2>/dev/null || true; \
			\
			echo "glibc" > /output/.libc-type; \
			;; \
		*) \
			echo "amd64/arm64: will use musl-based static binaries"; \
			echo "musl" > /output/.libc-type; \
			# Create placeholders so COPY doesn't fail \
			touch /output/lib/.placeholder; \
			touch /output/lib64/.placeholder; \
			;; \
	esac

#### kata-deploy main image
# distroless does not publish pinned/versioned tags - only rolling ones
# (latest, nonroot, debug) - so :latest is the intended way to consume it.
# hadolint ignore=DL3007
FROM gcr.io/distroless/static-debian13:latest

ARG DESTINATION=/opt/kata-artifacts

# Copy kata component tarballs and component manifest
COPY --from=artifact-stage ${DESTINATION} ${DESTINATION}

# Copy Rust binary
COPY --from=artifact-stage /opt/prebuilt/usr/bin/kata-deploy /usr/bin/kata-deploy

# Copy required libraries (assembled based on architecture)
COPY --from=runtime-assembler /output/lib/ /lib/
COPY --from=runtime-assembler /output/lib64/ /lib64/

# Copy the SELinux policy module, as source CIL so the node's own secilc
# compiles it and one file works across selinux-policy versions.
COPY tools/packaging/kata-deploy/selinux/kata-deploy.cil ${DESTINATION}/selinux/

# Copy nydus snapshotter
COPY tools/packaging/kata-deploy/nydus-snapshotter ${DESTINATION}/nydus-snapshotter
COPY --from=artifact-stage /opt/prebuilt/opt/kata-artifacts/nydus-snapshotter/containerd-nydus-grpc ${DESTINATION}/nydus-snapshotter/
COPY --from=artifact-stage /opt/prebuilt/opt/kata-artifacts/nydus-snapshotter/nydus-overlayfs ${DESTINATION}/nydus-snapshotter/

ENTRYPOINT ["/usr/bin/kata-deploy"]
