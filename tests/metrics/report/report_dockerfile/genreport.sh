#!/bin/bash
# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

REPORTNAME="metrics_report.pdf"

cd scripts

Rscript --slave -e "library(knitr);knit('pdf.Rmd')"
Rscript --slave -e "library(knitr);pandoc('pdf.md', format='latex')"

Rscript --slave -e "library(knitr);knit('html.Rmd')"
Rscript --slave -e "library(knitr);pandoc('html.md', format='html')"

cp /scripts/pdf.pdf /outputdir/${REPORTNAME}
cp /scripts/figure/*.png /outputdir/
echo "PNGs of graphs and tables can be found in the output directory."
echo "The report, named ${REPORTNAME}, can be found in the output directory"
