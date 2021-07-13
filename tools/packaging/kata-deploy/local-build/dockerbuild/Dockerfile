FROM ubuntu
ENV DEBIAN_FRONTEND=noninteractive

ADD install_yq.sh /usr/bin/install_yq.sh


# yq installer deps
RUN apt update && apt-get install -y curl sudo

# Install yq
RUN install_yq.sh

RUN curl -fsSL https://get.docker.com -o get-docker.sh
RUN sh get-docker.sh

ARG IMG_USER=kata-builder
ARG UID=1000
ARG GID=1000
RUN if [ ${IMG_USER} != "root" ]; then adduser ${IMG_USER} --uid=${UID};fi
RUN sh -c "echo '${IMG_USER} ALL=NOPASSWD: ALL' >> /etc/sudoers"

RUN apt-get update && \
  apt install -y \
  gcc \
  git \
  make \
  xz-utils

ENV USER ${IMG_USER}
USER ${UID}:${GID}
