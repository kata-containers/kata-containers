#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

REPORTNAME="metrics_report.pdf"

cd scripts

Rscript --slave -e "library(knitr);knit('metrics_report.Rmd')"
Rscript --slave -e "library(knitr);pandoc('metrics_report.md', format='latex')"

cp /scripts/${REPORTNAME} /outputdir
echo "The report, named ${REPORTNAME}, can be found in the output directory"
