ARG SUSE_VERSION=${SUSE_VERSION:-15.1}
FROM opensuse/leap:${SUSE_VERSION}


# Get OBS client, plugins and dependencies
RUN zypper -v -n install osc-plugin-install vim curl bsdtar git sudo
RUN zypper -v -n install build \
    obs-service-tar_scm \
    obs-service-verify_file \
    obs-service-obs_scm \
    obs-service-recompress \
    obs-service-download_url
