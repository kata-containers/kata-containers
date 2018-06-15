FROM opensuse:leap

ARG GO_VERSION=${GO_VERSION:-1.10.2}
ARG SUSE_VERSION=${SUSE_VERSION:-42.3}
ARG GO_ARCH=${GO_ARCH:-amd64}

# Get OBS client, plugins and dependencies
RUN zypper -n install osc-plugin-install vim curl bsdtar git sudo pcre-tools
RUN curl -OkL https://download.opensuse.org/repositories/openSUSE:Tools/openSUSE_${SUSE_VERSION}/openSUSE:Tools.repo
RUN zypper -n addrepo openSUSE:Tools.repo
RUN zypper --gpg-auto-import-keys refresh
RUN zypper -n install build \
    obs-service-tar_scm \
    obs-service-verify_file \
    obs-service-obs_scm \
    obs-service-recompress \
    obs-service-download_url

# Set Go environment
RUN curl -OL https://dl.google.com/go/go${GO_VERSION}.linux-${GO_ARCH}.tar.gz
RUN tar -C /usr/local -xzf go${GO_VERSION}.linux-${GO_ARCH}.tar.gz

# Local build dependencies
RUN zypper -n install make gcc yum xz
