FROM centos/systemd
ARG KATA_VER
ARG ARCH=x86_64
ARG KUBE_ARCH=amd64
ARG KATA_URL=https://github.com/kata-containers/runtime/releases/download/${KATA_VER}
ARG KATA_FILE=kata-static-${KATA_VER}-${ARCH}.tar.xz

RUN \
yum install -y epel-release && \
yum install -y bzip2 jq && \
curl -sOL ${KATA_URL}/${KATA_FILE} && \
mkdir -p /opt/kata-artifacts && \
tar xvf ${KATA_FILE} -C /opt/kata-artifacts/ && \
chown -R root:root /opt/kata-artifacts/ && \
rm ${KATA_FILE}

RUN \
curl -Lso /bin/kubectl https://storage.googleapis.com/kubernetes-release/release/$(curl -s https://storage.googleapis.com/kubernetes-release/release/stable.txt)/bin/linux/${KUBE_ARCH}/kubectl && \
chmod +x /bin/kubectl

COPY scripts /opt/kata-artifacts/scripts
RUN \
ln -s /opt/kata-artifacts/scripts/kata-deploy-docker.sh /usr/bin/kata-deploy-docker && \
ln -s /opt/kata-artifacts/scripts/kata-deploy.sh /usr/bin/kata-deploy
