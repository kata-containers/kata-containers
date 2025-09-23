FROM ubuntu:24.04

ENV ARCH="x86_64"
ENV DEBUG="1"
ENV VARIANT="nvidia-gpu"
ENV NVIDIA_GPU_STACK="compute,driver,nvswitch,version=580"
ENV ROOTFS_OS="ubuntu"
ENV OS_VERSION="noble"

ENV KATA_REPO_VERSION="main"
ENV OUTPUT_DIR="/output"

RUN apt update
RUN apt install -y git
RUN apt install -y sudo
RUN apt install -y build-essential
RUN apt install -y mmdebstrap
RUN apt install -y makedev
RUN apt install -y curl
RUN apt install -y musl musl-dev musl-tools
RUN apt install -y golang-go
RUN apt install -y zstd

# Pull in our local files
COPY ./run.sh /input/
COPY ./kernel/ /input/kernel/

# Output is placed in the /output directory.
# We could make this a VOLUME to force it to be attached to the host, but let's
# just leave it as a container dir that can then be over-ridden from a host commandline
# volume setup.
# VOLUME /output

# By default build everything
CMD ["/input/run.sh"]
