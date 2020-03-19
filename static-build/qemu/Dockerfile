from ubuntu:18.04

ARG QEMU_REPO
# commit/tag/branch
ARG QEMU_VERSION
ARG QEMU_TARBALL
ARG PREFIX

WORKDIR /root/qemu
RUN apt-get update && apt-get upgrade -y
RUN apt-get --no-install-recommends install -y \
	    apt-utils \
	    autoconf \
	    automake \
	    bc \
	    bison \
	    ca-certificates \
	    cpio \
	    flex \
	    gawk \
	    libaudit-dev \
	    libcap-dev \
	    libcap-ng-dev \
	    libdw-dev \
	    libelf-dev \
	    libglib2.0-0 \
	    libglib2.0-dev \
	    libglib2.0-dev git \
	    libltdl-dev \
	    libpixman-1-dev \
	    libpmem-dev \
	    libtool \
	    pkg-config \
	    pkg-config \
	    python \
	    python-dev \
	    rsync \
	    zlib1g-dev

RUN cd  .. && git clone "${QEMU_REPO}" qemu
RUN git checkout "${QEMU_VERSION}"
RUN git clone https://github.com/qemu/capstone.git capstone
RUN git clone https://github.com/qemu/keycodemapdb.git ui/keycodemapdb

ADD scripts/configure-hypervisor.sh /root/configure-hypervisor.sh
ADD qemu/patches/ /root/kata_qemu_patches

RUN  \
	cat VERSION; \
	stable_branch=$(cat VERSION | awk 'BEGIN{FS=OFS="."}{print $1 "." $2 ".x"}');\
	for patch in $(find /root/kata_qemu_patches/${stable_branch}/ -name '*.patch'); do\
		echo "apply $patch";\
		git apply "$patch"; \
	done

RUN PREFIX="${PREFIX}" /root/configure-hypervisor.sh -s kata-qemu | xargs ./configure \
	--with-pkgversion=kata-static

RUN make -j$(nproc)
RUN make install DESTDIR=/tmp/qemu-static
RUN cd /tmp/qemu-static && tar -czvf "${QEMU_TARBALL}" *
