FROM centos/systemd
ARG KATA_VER=1.1.0
ARG KATA_URL=https://github.com/kata-containers/runtime/releases/download/${KATA_VER}
ARG KUBECTL_VER=v1.10.2

RUN \
curl -sOL ${KATA_URL}/kata-release-binaries.tar.xz && \
mkdir -p /opt/kata-artifacts && \
tar xvf kata-release-binaries.tar.xz -C /opt/kata-artifacts && \
rm kata-release-binaries.tar.xz

RUN \
curl -s -o /bin/kubectl https://storage.googleapis.com/kubernetes-release/release/${KUBECTL_VER}/bin/linux/amd64/kubectl && \
chmod +x /bin/kubectl

COPY scripts /opt/kata-artifacts/scripts
