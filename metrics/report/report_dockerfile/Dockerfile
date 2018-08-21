# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Set up an Ubuntu image with the components needed to generate a
# metrics report. That includes:
#  - R
#  - The R 'tidyverse'
#  - pandoc
#  - The report generation R files and helper scripts

# I would have liked to have used from base rocker/verse, but that
# does not have all the tools we need, and it seems that apt install
# does not simply work - texlive for instance did not install.
#
# So, build up our image by hand then...
FROM ubuntu

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

# Without this some of the package installs stop to try and ask questions...
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
	apt-get install -y tzdata r-base pandoc texlive

# R packages get built from source, and can take some time, so split them
# into individual RUNs so we can make some later modifications without maybe taking
# the full image rebuild hit.
RUN Rscript -e "install.packages('tidyverse')"
RUN Rscript -e "install.packages('knitr')"
RUN Rscript -e "install.packages('gridExtra')"
RUN Rscript -e "install.packages('ggpubr')"

# Pull in our actual worker scripts
COPY . /scripts

# By default generate the report
CMD ["/scripts/genreport.sh"]
