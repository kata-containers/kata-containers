FROM centos/systemd
ARG KATA_VER=1.4.0
ARG ARCH=x86_64
ARG KATA_URL=https://github.com/kata-containers/runtime/releases/download/${KATA_VER}
ARG KATA_FILE=kata-static-${KATA_VER}-${ARCH}.tar.xz
ARG KUBECTL_VER=v1.10.2

RUN \
curl -sOL ${KATA_URL}/${KATA_FILE} && \
mkdir -p /opt/kata-artifacts && \
tar xvf ${KATA_FILE} -C /opt/kata-artifacts/ && \
rm ${KATA_FILE}

RUN \
curl -s -o /bin/kubectl https://storage.googleapis.com/kubernetes-release/release/${KUBECTL_VER}/bin/linux/amd64/kubectl && \
chmod +x /bin/kubectl

COPY scripts /opt/kata-artifacts/scripts
