FROM ubuntu:latest

LABEL version="0.0.0"
LABEL maintainer="Kata folks"
LABEL com.github.actions.name="Prepare artifacts for Kata release page"
LABEL com.github.actions.description="Create and upload static binaries and Kata images to release page for a given release"

ENV GITHUB_ACTION_NAME="Prepare artifacts for Kata release"
ENV NEW_VERSION="1.8.2"
ENV BRANCH="master"

RUN git clone https://github.com/kata-containers/packaging.git && cd packaging

ENTRYPOINT ["/entrypoint.sh"]
