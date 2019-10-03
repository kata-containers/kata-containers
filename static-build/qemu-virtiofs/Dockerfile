from ubuntu:18.04

ARG QEMU_VIRTIOFS_REPO
# commit/tag/branch
ARG QEMU_VIRTIOFS_TAG
ARG PREFIX

WORKDIR /root/qemu-virtiofs
RUN apt-get update && apt-get upgrade -y
RUN apt-get install -y \
	    autoconf \
	    automake \
	    bc \
	    bison \
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
	    libseccomp-dev \
	    libtool \
	    patch \
	    pkg-config \
	    pkg-config \
	    python \
	    python-dev \
	    rsync \
	    seccomp \
	    libseccomp2 \
	    zlib1g-dev

RUN cd  .. && git clone "${QEMU_VIRTIOFS_REPO}" qemu-virtiofs
RUN git checkout "${QEMU_VIRTIOFS_TAG}"
ADD qemu/patches/virtiofsd/0001-add-time-to-seccomp.patch /root/0001-add-time-to-seccomp.patch
RUN patch -p1 < /root/0001-add-time-to-seccomp.patch
ADD scripts/configure-hypervisor.sh /root/configure-hypervisor.sh
RUN PREFIX="${PREFIX}" /root/configure-hypervisor.sh -s kata-qemu | sed -e 's|--enable-rbd||g' -e 's|--disable-seccomp||g' | xargs ./configure \
	--with-pkgversion=kata-static
RUN make -j$(nproc)
RUN make -j$(nproc) virtiofsd
RUN make install DESTDIR=/tmp/qemu-virtiofs-static
RUN mv /tmp/qemu-virtiofs-static/"${PREFIX}"/bin/qemu-system-x86_64 /tmp/qemu-virtiofs-static/"${PREFIX}"/bin/qemu-virtiofs-system-x86_64
RUN chmod +x virtiofsd && mv virtiofsd /tmp/qemu-virtiofs-static/opt/kata/bin/
RUN cd /tmp/qemu-virtiofs-static && tar -czvf kata-qemu-static.tar.gz *
