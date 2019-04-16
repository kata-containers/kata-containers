FROM opensuse:leap

ARG SUSE_VERSION=${SUSE_VERSION:-42.3}

# Get OBS client, plugins and dependencies
RUN zypper -v -n install osc-plugin-install vim curl bsdtar git sudo
RUN curl -OkL https://download.opensuse.org/repositories/openSUSE:Tools/openSUSE_${SUSE_VERSION}/openSUSE:Tools.repo
RUN zypper -n addrepo openSUSE:Tools.repo
RUN zypper --gpg-auto-import-keys refresh
RUN zypper -v -n install build \
    obs-service-tar_scm \
    obs-service-verify_file \
    obs-service-obs_scm \
    obs-service-recompress \
    obs-service-download_url
