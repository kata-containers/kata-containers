# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Set up an Ubuntu image with the components needed to generate a
# metrics report. That includes:
#  - R
#  - The R 'tidyverse'
#  - pandoc
#  - The report generation R files and helper scripts

# Start with the base rocker tidyverse.
# We would have used the 'verse' base, that already has some of the docs processing
# installed, but I could not figure out how to add in the extra bits we needed to
# the lite tex version is uses.
# Here we specify a tag for base image instead of using latest to let it free from
# the risk from the update of latest base image.
FROM rocker/tidyverse:3.6.0

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.2"

# Without this some of the package installs stop to try and ask questions...
ENV DEBIAN_FRONTEND=noninteractive

# Install the extra doc processing parts we need for our Rmarkdown PDF flow.
RUN apt-get update -qq && \
  apt-get install -y --no-install-recommends \
    texlive-latex-base \
    texlive-fonts-recommended \
    latex-xcolor && \
  apt-get clean && \
  rm -rf /var/lib/apt/lists

# Install the extra R packages we need.
RUN install2.r --error --deps TRUE \
	gridExtra \
	ggpubr

# Pull in our actual worker scripts
COPY . /scripts

# By default generate the report
CMD ["/scripts/genreport.sh"]
